use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{Field, Fields, ItemEnum, ItemStruct, Type};

pub(crate) fn ast_node_struct(input: &ItemStruct) -> TokenStream {
    // Ensure it's a named-field struct (not tuple or unit)
    let mut new_struct = input.clone();
    match &mut new_struct.fields {
        Fields::Named(fields_named) => {
            for field in &fields_named.named {
                match &field.ident {
                    Some(ident) if ident == "node_id" => {
                        let err = syn::Error::new_spanned(
                            field,
                            "ast_node reserves the 'node_id' field name",
                        )
                        .to_compile_error();
                        return err.into();
                    }
                    _ => {}
                }

                match field.vis {
                    syn::Visibility::Public(_) => {}
                    _ => {
                        let err =
                            syn::Error::new_spanned(field, "all members of ast_node must be 'pub'")
                                .to_compile_error();
                        return err.into();
                    }
                }
            }

            let node_id_field_ident = format_ident!("node_id");
            let node_id_field_type: Type = syn::parse_quote!(fpp_core::Node);
            let node_id_field = Field {
                attrs: vec![syn::parse_quote!(#[serde(
                    rename = "annotations",
                    serialize_with = "crate::serde_annotate::serialize"
                )])],
                vis: syn::Visibility::Public(syn::parse_quote!(pub)),
                mutability: syn::FieldMutability::None,
                ident: Some(node_id_field_ident.clone()),
                colon_token: Some(<syn::Token![:]>::default()),
                ty: node_id_field_type,
            };
            fields_named.named.push(node_id_field);
        }
        Fields::Unit | Fields::Unnamed(_) => {
            // Return a compile_error if user applied macro on unsupported struct
            let err = syn::Error::new_spanned(
                input,
                "#[ast_node]
#[derive(AstAnnotated)] only supports structs with named fields (braced structs).",
            )
            .to_compile_error();
            return err.into();
        }
    }

    let struct_ident = &new_struct.ident;
    let generics = &new_struct.generics;
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let output = quote! {
        #new_struct

        impl #impl_generics ::fpp_core::Spanned for #struct_ident #ty_generics #where_clause {
            fn span(&self) -> ::fpp_core::Span {
                fpp_core::Spanned::span(&self.node_id)
            }
        }

        impl #impl_generics crate::AstNode for #struct_ident #ty_generics #where_clause {
            fn id(&self) -> fpp_core::Node {
                self.node_id
            }
        }

        impl #impl_generics ::core::cmp::PartialEq for #struct_ident #ty_generics #where_clause {
            fn eq(&self, other: &Self) -> bool {
                self.id() == other.id()
            }
        }

        impl #impl_generics ::core::cmp::Eq for #struct_ident #ty_generics #where_clause {}

        impl #impl_generics ::core::hash::Hash for #struct_ident #ty_generics #where_clause {
            fn hash<H: ::std::hash::Hasher>(&self, state: &mut H) {
                self.id().hash(state)
            }
        }
    };

    output.into()
}

pub(crate) fn ast_node_enum(input: &ItemEnum) -> TokenStream {
    let enum_ident = &input.ident;
    let generics = &input.generics;
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    // Build match arms for variants that have exactly one unnamed field.
    let mut span_arms = vec![];
    let mut id_arms = vec![];
    let mut debug_arms = vec![];

    for v in &input.variants {
        let var_ident = &v.ident;
        match &v.fields {
            Fields::Unnamed(fields) if fields.unnamed.len() == 1 => {
                span_arms.push(quote! { #enum_ident::#var_ident(inner) => inner.span(), });
                id_arms.push(quote! { #enum_ident::#var_ident(inner) => inner.id(), });
                debug_arms.push(quote! { #enum_ident::#var_ident(inner) => inner.fmt(f), });
            }
            _ => {
                // Return a compile_error if user applied macro on unsupported struct
                let err = syn::Error::new_spanned(
                    var_ident,
                    format!(
                        "Variant {:?} does not wrap a type implementing AstNode",
                        var_ident
                    ),
                )
                .to_compile_error();
                return err.into();
            }
        }
    }

    let output = quote! {

        #input

        impl #impl_generics fpp_core::Spanned for #enum_ident #ty_generics #where_clause {
            fn span(&self) -> fpp_core::Span {
                match self {
                    #( #span_arms )*
                }
            }
        }

        impl #impl_generics crate::AstNode for #enum_ident #ty_generics #where_clause {
            fn id(&self) -> fpp_core::Node {
                match self {
                    #( #id_arms )*
                }
            }
        }

        impl std::fmt::Debug for #enum_ident #ty_generics #where_clause {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                match self {
                    #( #debug_arms )*
                }
            }
        }
    };

    output.into()
}
