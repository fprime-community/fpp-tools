use quote::{quote, quote_spanned};
use syn::spanned::Spanned;

pub(super) fn replicate_derive(mut s: synstructure::Structure<'_>) -> proc_macro2::TokenStream {
    if let syn::Data::Union(_) = s.ast().data {
        panic!("cannot derive on union")
    }

    s.add_bounds(synstructure::AddBounds::Generics);
    s.bind_with(|_| synstructure::BindStyle::RefMut);

    // Every field is replicated, including the ones a visitor skips: a copy of a
    // subtree must not share any node identity with the original, and
    // `#[visitable(ignore)]` fields can carry node identities too.
    let body = s.each(|bind| {
        let span = bind.ast().span();
        quote_spanned! { span => crate::replicate::Replicate::replicate(#bind, __r) }
    });

    s.gen_impl(quote! {
        gen impl crate::replicate::Replicate for @Self {
            fn replicate(&mut self, __r: &crate::replicate::Replicator) {
                match *self { #body }
            }
        }
    })
}
