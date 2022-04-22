pub mod align;
pub mod display;
pub mod size;

use crate::workspace::{BindingInfoId, ModuleId};
use chili_span::Span;
use std::{fmt, ops::Deref};
use ustr::{ustr, Ustr, UstrMap};

#[derive(Debug, Hash, PartialEq, Eq, Clone, Copy)]
pub struct Ty(pub usize);

impl Default for Ty {
    fn default() -> Self {
        Self(usize::MAX)
    }
}

impl From<Ty> for TyKind {
    fn from(val: Ty) -> Self {
        TyKind::Var(val)
    }
}

impl Ty {
    pub fn unknown() -> Self {
        Default::default()
    }

    pub fn kind(&self) -> TyKind {
        (*self).into()
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum TyKind {
    Never,
    Unit,
    Bool,
    Int(IntTy),
    UInt(UIntTy),
    Float(FloatTy),
    Pointer(Box<TyKind>, bool),
    MultiPointer(Box<TyKind>, bool),
    Fn(FnTy),
    Array(Box<TyKind>, usize),
    Slice(Box<TyKind>, bool),
    Tuple(Vec<TyKind>),
    Struct(StructTy),
    Module(ModuleId),
    Type(Box<TyKind>),
    Var(Ty),
    Infer(Ty, InferTy),
    Unknown,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum InferTy {
    AnyInt,
    AnyFloat,
    PartialStruct(PartialStructTy),
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum IntTy {
    I8,
    I16,
    I32,
    I64,
    Int,
}

impl Default for IntTy {
    fn default() -> Self {
        IntTy::I32
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum UIntTy {
    U8,
    U16,
    U32,
    U64,
    UInt,
}

impl Default for UIntTy {
    fn default() -> Self {
        UIntTy::U32
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum FloatTy {
    F16,
    F32,
    F64,
    Float,
}

impl Default for FloatTy {
    fn default() -> Self {
        FloatTy::F32
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct FnTy {
    pub params: Vec<TyKind>,
    pub ret: Box<TyKind>,
    pub variadic: bool,
    pub lib_name: Option<Ustr>,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct StructTy {
    pub name: Ustr,
    pub binding_info_id: BindingInfoId,
    pub fields: Vec<StructTyField>,
    pub kind: StructTyKind,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct PartialStructTy(UstrMap<TyKind>);

impl Deref for PartialStructTy {
    type Target = UstrMap<TyKind>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl PartialStructTy {
    pub fn into_struct(&self) -> StructTy {
        StructTy {
            name: ustr(""),
            binding_info_id: Default::default(),
            fields: self
                .iter()
                .map(|(&symbol, ty)| StructTyField {
                    symbol,
                    ty: ty.clone(),
                    span: Span::unknown(),
                })
                .collect(),
            kind: StructTyKind::Struct,
        }
    }
}

impl From<StructTy> for TyKind {
    fn from(ty: StructTy) -> Self {
        TyKind::Struct(ty)
    }
}

impl StructTy {
    pub fn is_struct(&self) -> bool {
        matches!(self.kind, StructTyKind::Struct)
    }

    pub fn is_packed_struct(&self) -> bool {
        matches!(self.kind, StructTyKind::PackedStruct)
    }

    pub fn is_union(&self) -> bool {
        matches!(self.kind, StructTyKind::Union)
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum StructTyKind {
    Struct,
    PackedStruct,
    Union,
}

impl StructTy {
    pub fn opaque(name: Ustr, binding_info_id: BindingInfoId, kind: StructTyKind) -> Self {
        Self {
            name,
            binding_info_id,
            fields: vec![],
            kind,
        }
    }

    pub fn temp(fields: Vec<StructTyField>, kind: StructTyKind) -> Self {
        Self {
            name: ustr(""),
            binding_info_id: Default::default(),
            fields,
            kind,
        }
    }

    pub fn is_anonymous(&self) -> bool {
        self.name.is_empty()
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct StructTyField {
    pub symbol: Ustr,
    pub ty: TyKind,
    pub span: Span,
}

impl StructTyField {
    pub fn temp(ty: TyKind) -> Self {
        Self {
            symbol: ustr(""),
            ty,
            span: Span::unknown(),
        }
    }
}

impl From<TyKind> for String {
    fn from(val: TyKind) -> Self {
        val.to_string()
    }
}

impl TyKind {
    pub fn is_type(&self) -> bool {
        matches!(self, TyKind::Type(_))
    }

    pub fn is_module(&self) -> bool {
        matches!(self, TyKind::Module(_))
    }

    pub fn is_number(&self) -> bool {
        self.is_any_integer() || self.is_float()
    }

    pub fn is_any_integer(&self) -> bool {
        self.is_int() || self.is_uint() || self.is_anyint()
    }

    pub fn is_anyint(&self) -> bool {
        matches!(self, TyKind::Infer(_, InferTy::AnyInt))
    }

    pub fn is_anyfloat(&self) -> bool {
        matches!(self, TyKind::Infer(_, InferTy::AnyFloat))
    }

    pub fn is_int(&self) -> bool {
        matches!(self, TyKind::Int(_))
    }

    pub fn is_uint(&self) -> bool {
        matches!(self, TyKind::UInt(_))
    }

    pub fn is_float(&self) -> bool {
        matches!(self, TyKind::Float(_))
    }

    pub fn is_pointer(&self) -> bool {
        matches!(self, TyKind::Pointer(..))
    }

    pub fn is_multi_pointer(&self) -> bool {
        matches!(self, TyKind::MultiPointer(..))
    }

    pub fn is_bool(&self) -> bool {
        matches!(self, TyKind::Bool)
    }

    pub fn is_fn(&self) -> bool {
        matches!(self, TyKind::Fn(..))
    }

    pub fn is_var(&self) -> bool {
        matches!(self, TyKind::Var(..))
    }

    pub fn is_array(&self) -> bool {
        matches!(self, TyKind::Array(..))
    }

    pub fn is_slice(&self) -> bool {
        matches!(self, TyKind::Slice(..))
    }

    pub fn is_unknown(&self) -> bool {
        matches!(self, TyKind::Unknown)
    }

    pub fn is_unit(&self) -> bool {
        matches!(self, TyKind::Unit)
    }

    pub fn is_never(&self) -> bool {
        matches!(self, TyKind::Never)
    }

    pub fn is_struct(&self) -> bool {
        matches!(self, TyKind::Struct(_))
    }

    pub fn is_aggregate(&self) -> bool {
        self.is_array() || self.is_struct()
    }

    pub fn into_struct(&self) -> &StructTy {
        match self {
            TyKind::Struct(ty) => ty,
            _ => panic!("expected struct, got {:?}", self),
        }
    }

    pub fn as_fn(&self) -> &FnTy {
        match self {
            TyKind::Fn(ty) => ty,
            _ => panic!("expected fn, got {:?}", self),
        }
    }

    pub fn into_fn(self) -> FnTy {
        match self {
            TyKind::Fn(ty) => ty,
            _ => panic!("expected fn, got {:?}", self),
        }
    }
}

impl TyKind {
    pub fn raw_pointer(is_mutable: bool) -> TyKind {
        TyKind::Pointer(Box::new(TyKind::Int(IntTy::I8)), is_mutable)
    }

    pub fn str() -> TyKind {
        TyKind::Slice(Box::new(TyKind::char()), false)
    }

    pub fn char() -> TyKind {
        TyKind::UInt(UIntTy::U8)
    }

    pub fn create_type(self) -> TyKind {
        TyKind::Type(Box::new(self))
    }

    pub fn element_type(&self) -> Option<&TyKind> {
        match self {
            TyKind::Pointer(inner, _)
            | TyKind::MultiPointer(inner, _)
            | TyKind::Array(inner, _)
            | TyKind::Slice(inner, _)
            | TyKind::Type(inner) => Some(inner),
            _ => None,
        }
    }

    pub fn pointer_type(self, mutable: bool) -> TyKind {
        TyKind::Pointer(Box::new(self), mutable)
    }

    pub fn maybe_deref_once(&self) -> TyKind {
        match self {
            TyKind::Pointer(inner, _) => inner.as_ref().clone(),
            _ => self.clone(),
        }
    }
}
