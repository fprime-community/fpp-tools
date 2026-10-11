use crate::semantics::{
    AbsType, AliasType, AnonArrayType, AnonStructType, ArrayType, EnumType, StructType, Type,
};
use fpp_ast::{FloatKind, IntegerKind};
use std::sync::Arc;

/// Visit a type
///
/// Each case of [`Type`] has a hook, which by default calls
/// [`TypeVisitor::default`]; a visitor overrides the hooks of the cases it
/// handles.
pub(crate) trait TypeVisitor {
    /// The input of the visitor
    type In<'a>;

    /// The output of the visitor
    type Out;

    fn abs_type(&self, input: Self::In<'_>, t: &Arc<AbsType>) -> Self::Out {
        self.default(input, &Type::Abs(t.clone()))
    }

    fn alias_type(&self, input: Self::In<'_>, t: &AliasType) -> Self::Out {
        self.default(input, &Type::Alias(t.clone()))
    }

    fn anon_array(&self, input: Self::In<'_>, t: &AnonArrayType) -> Self::Out {
        self.default(input, &Type::AnonArray(t.clone()))
    }

    fn anon_struct(&self, input: Self::In<'_>, t: &AnonStructType) -> Self::Out {
        self.default(input, &Type::AnonStruct(t.clone()))
    }

    fn array(&self, input: Self::In<'_>, t: &Arc<ArrayType>) -> Self::Out {
        self.default(input, &Type::Array(t.clone()))
    }

    fn boolean(&self, input: Self::In<'_>) -> Self::Out {
        self.default(input, &Type::Boolean)
    }

    fn default(&self, input: Self::In<'_>, t: &Type) -> Self::Out;

    fn enumeration(&self, input: Self::In<'_>, t: &Arc<EnumType>) -> Self::Out {
        self.default(input, &Type::Enum(t.clone()))
    }

    fn float(&self, input: Self::In<'_>, kind: FloatKind) -> Self::Out {
        self.default(input, &Type::Float(kind))
    }

    fn integer(&self, input: Self::In<'_>) -> Self::Out {
        self.default(input, &Type::Integer)
    }

    fn primitive_int(&self, input: Self::In<'_>, kind: IntegerKind) -> Self::Out {
        self.default(input, &Type::PrimitiveInt(kind))
    }

    fn string(&self, input: Self::In<'_>, size: Option<i128>) -> Self::Out {
        self.default(input, &Type::String(size))
    }

    fn struct_(&self, input: Self::In<'_>, t: &Arc<StructType>) -> Self::Out {
        self.default(input, &Type::Struct(t.clone()))
    }

    fn ty(&self, input: Self::In<'_>, t: &Type) -> Self::Out {
        self.match_type(input, t)
    }

    /// Dispatch a type to the hook of its case. Visitors do not override this.
    fn match_type(&self, input: Self::In<'_>, t: &Type) -> Self::Out {
        match t {
            Type::Abs(t) => self.abs_type(input, t),
            Type::Alias(t) => self.alias_type(input, t),
            Type::AnonArray(t) => self.anon_array(input, t),
            Type::AnonStruct(t) => self.anon_struct(input, t),
            Type::Array(t) => self.array(input, t),
            Type::Boolean => self.boolean(input),
            Type::Enum(t) => self.enumeration(input, t),
            Type::Float(kind) => self.float(input, *kind),
            Type::Integer => self.integer(input),
            Type::PrimitiveInt(kind) => self.primitive_int(input, *kind),
            Type::String(size) => self.string(input, *size),
            Type::Struct(t) => self.struct_(input, t),
        }
    }
}
