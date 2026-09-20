mod annotated;
mod enum_map;
mod node;
mod util;
mod visitable;

use crate::annotated::{annotated_enum, annotated_struct};
use crate::enum_map::enum_map;
use crate::node::{ast_node_enum, ast_node_struct};
use proc_macro::TokenStream;
use syn::{Item, parse_macro_input};
use synstructure::decl_derive;

///
/// Defines an AstNode from struct or enum
///
/// For structs, an additional field is added and traits are implemented.
///
/// The following checks are performed on structs:
/// 1. No field in the struct definition is named 'node_id'
/// 2. All fields are 'pub'
///
/// Enums require all variants to be AstNodes
///
/// A struct using this attribute must also derive `serde::Serialize`: the injected
/// `node_id` field serializes as `annotations`, via `crate::serde_annotate::serialize`.
///
/// # Examples
///
/// For structures:
/// ```ignore
/// use fpp_macros::ast_node;
///
/// #[ast_node]
/// pub struct TlmChannelIdentifier {
///    pub component_instance: QualIdent,
///    pub channel_name: Ident,
/// }
/// ```
///
/// For enums:
/// ```ignore
/// use fpp_macros::ast;
///
/// #[ast]
/// pub enum InterfaceMember {
///     SpecPortInstance(SpecPortInstance),
///     SpecImport(SpecImport),
/// }
/// ```
#[proc_macro_attribute]
pub fn ast(_attrs: TokenStream, input: TokenStream) -> TokenStream {
    // Parse the input tokens into a syntax tree
    let item = parse_macro_input!(input);

    match &item {
        Item::Struct(item_struct) => ast_node_struct(item_struct),
        Item::Enum(item_enum) => ast_node_enum(item_enum),
        other => {
            let err = syn::Error::new_spanned(
                other,
                "#[ast_node] #[derive(AstAnnotated)] only supports structs or enums",
            )
            .to_compile_error();
            err.into()
        }
    }
}

///
/// Derives wrapper trait for accessing ast node annotation which are
/// interned in the compiler context.
///
/// The following pre-requisites are checked:
/// 1. Struct or Enum
/// 2. Already inherits #[ast]
/// 3. If enum, all variants also derive from AstAnnotated
///
/// For structs
/// ```ignore
/// #[ast]
/// #[derive(AstAnnotated)]
/// pub struct SpecStateMachineInstance {
///     pub name: Ident,
///     pub state_machine: QualIdent,
///     pub priority: Option<Expr>,
///     pub queue_full: Option<QueueFull>,
/// }
/// ```
///
/// For enums:
/// ```ignore
/// use fpp_macros::ast_node;
///
/// #[ast]
/// #[derive(AstAnnotated)]
/// pub enum InterfaceMember {
///     SpecPortInstance(SpecPortInstance),
///     SpecImport(SpecImport),
/// }
/// ```
#[proc_macro_derive(AstAnnotated)]
pub fn ast_annotated(input: TokenStream) -> TokenStream {
    // Parse the input tokens into a syntax tree
    let item = parse_macro_input!(input);

    match item {
        Item::Struct(item_struct) => annotated_struct(&item_struct),
        Item::Enum(item_enum) => annotated_enum(&item_enum),
        other => {
            let err = syn::Error::new_spanned(other, "#[ast_node] only supports structs or enums")
                .to_compile_error();
            err.into()
        }
    }
}

decl_derive!(
    [VisitorWalkable, attributes(visitable)] =>
    /// Derives `Walkable + Visitable` for the annotated `struct` or `enum` (`union` is not supported).
    ///
    /// For `Walkable`:
    /// Each field of the struct or enum variant will be visited in definition order, using the
    /// `Visitable` implementation for its type. However, if a field of a struct or an enum
    /// variant is annotated with `#[visitable(ignore)]` then that field will not be
    /// visited (and its type is not required to implement `Walkable`).
    ///
    /// For `Visitable`:
    /// The visitor's callback for this node type will be called. The function will be called
    /// `visit_<type_name_in_snake_case>. For example, for the type DefAliasType, the
    /// [fpp_ast::Visitor::visit_def_alias_type] will be called.
    visitable::walkable_visit_derive
);

decl_derive!(
    [DirectWalkable, attributes(visitable)] =>
    /// Derives `Walkable + Visitable` for the annotated `struct` or `enum` (`union` is not supported).
    ///
    /// This macro differs from [VisitorWalkable] as it does not delegate to the `Visitor`. Instead, it
    /// directly walks the children. This effectively means that the behavior cannot be overridden
    /// in the visitor and also avoid the need to have a signature in the Visitor to implement the
    /// walking behavior.
    ///
    /// Typically, this derivation should be used for the `*Member` or `*Kind` enums which just wrap
    /// different implementations of a type of node. Each of the variant nodes would be [VisitorWalkable]
    /// and therefore another trip through the visitor would be redundant.
    ///
    /// For `Walkable`:
    /// Each field of the struct or enum variant will be visited in definition order, using the
    /// `Walkable` implementation for its type. However, if a field of a struct or an enum
    /// variant is annotated with `#[visitable(ignore)]` then that field will not be
    /// visited (and its type is not required to implement `Walkable`).
    ///
    /// For `Visitable`:
    /// The type's [Walkable] implementation will be run
    visitable::walkable_direct_derive
);

decl_derive!(
    [DirectRefWalkable, attributes(visitable)] =>
    /// Same as [DirectWalkable] except it only generates
    visitable::walkable_direct_ref_derive
);

/// Count the number of variants in an enum and emit a `SIZE` constant in the impl
#[proc_macro_derive(EnumMap)]
pub fn enum_count(input: TokenStream) -> TokenStream {
    // Parse the input tokens into a syntax tree
    let item = parse_macro_input!(input);

    (match item {
        Item::Enum(input) => enum_map(input),
        other => {
            let err = syn::Error::new_spanned(other, "#[ast_node] only supports structs or enums")
                .to_compile_error();
            return err.into();
        }
    })
    .into()
}
