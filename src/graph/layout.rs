pub use grw_derive::Val;

pub const FNV_OFFSET: u64 = 14695981039346656037;
pub const FNV_PRIME: u64 = 1099511628211;

pub fn fnv_hash_bytes(mut hash: u64, bytes: &[u8]) -> u64 {
    for &b in bytes {
        hash = (hash ^ b as u64).wrapping_mul(FNV_PRIME);
    }
    hash
}

pub fn fnv_hash_u64(hash: u64, val: u64) -> u64 {
    fnv_hash_bytes(hash, &val.to_le_bytes())
}

pub fn fnv_hash_byte(hash: u64, byte: u8) -> u64 {
    (hash ^ byte as u64).wrapping_mul(FNV_PRIME)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EnumVariant {
    pub name: &'static str,
    pub discriminant: i128,
}

#[derive(Debug)]
pub struct EnumMeta {
    pub type_name: &'static str,
    pub variants: &'static [EnumVariant],
}

#[derive(Debug, Clone, Copy)]
pub enum FieldType {
    Bool,
    I8,
    I16,
    I32,
    I64,
    U8,
    U16,
    U32,
    U64,
    F32,
    F64,
    String,
    Struct(fn() -> &'static [FieldInfo]),
    Enum(&'static EnumMeta),
}

impl PartialEq for FieldType {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Bool, Self::Bool)
            | (Self::I8, Self::I8)
            | (Self::I16, Self::I16)
            | (Self::I32, Self::I32)
            | (Self::I64, Self::I64)
            | (Self::U8, Self::U8)
            | (Self::U16, Self::U16)
            | (Self::U32, Self::U32)
            | (Self::U64, Self::U64)
            | (Self::F32, Self::F32)
            | (Self::F64, Self::F64)
            | (Self::String, Self::String) => true,
            (Self::Struct(a), Self::Struct(b)) => std::ptr::fn_addr_eq(*a, *b),
            (Self::Enum(a), Self::Enum(b)) => std::ptr::eq(*a, *b),
            _ => false,
        }
    }
}

impl Eq for FieldType {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FieldInfo {
    pub name: &'static str,
    pub ty: FieldType,
    pub offset: usize,
}

pub struct MethodParam {
    pub name: &'static str,
    pub ty: FieldType,
}

unsafe impl Send for MethodParam {}
unsafe impl Sync for MethodParam {}

pub struct MethodMeta {
    pub name: &'static str,
    pub ret_type: FieldType,
    pub params: &'static [MethodParam],
    pub is_static: bool,
    pub fn_ptr: *const u8,
}

unsafe impl Send for MethodMeta {}
unsafe impl Sync for MethodMeta {}

#[doc(hidden)]
pub trait __GrwMethodFallback {
    fn __grw_method_table() -> &'static [MethodMeta] { &[] }
}

pub trait Val: 'static {
    fn fields() -> &'static [FieldInfo];
    fn methods() -> &'static [MethodMeta] { &[] }
    fn field_type() -> FieldType;
    fn layout_hash() -> u64;
    fn size() -> usize;
    fn align() -> usize;
}

impl Val for () {
    fn fields() -> &'static [FieldInfo] { &[] }
    fn field_type() -> FieldType { panic!("() cannot appear as a struct field") }
    fn layout_hash() -> u64 { 0x00 }
    fn size() -> usize { 0 }
    fn align() -> usize { 1 }
}

macro_rules! impl_val_primitive {
    ($ty:ty, $hash:expr, $ft:expr) => {
        impl Val for $ty {
            fn fields() -> &'static [FieldInfo] { &[] }
            fn field_type() -> FieldType { $ft }
            fn layout_hash() -> u64 { $hash }
            fn size() -> usize { std::mem::size_of::<$ty>() }
            fn align() -> usize { std::mem::align_of::<$ty>() }
        }
    };
}

impl_val_primitive!(bool, 0x01, FieldType::Bool);
impl_val_primitive!(i8, 0x02, FieldType::I8);
impl_val_primitive!(i16, 0x03, FieldType::I16);
impl_val_primitive!(i32, 0x04, FieldType::I32);
impl_val_primitive!(i64, 0x05, FieldType::I64);
impl_val_primitive!(u8, 0x06, FieldType::U8);
impl_val_primitive!(u16, 0x07, FieldType::U16);
impl_val_primitive!(u32, 0x08, FieldType::U32);
impl_val_primitive!(u64, 0x09, FieldType::U64);
impl_val_primitive!(f32, 0x0A, FieldType::F32);
impl_val_primitive!(f64, 0x0B, FieldType::F64);
impl_val_primitive!(String, 0x0C, FieldType::String);

#[macro_export]
macro_rules! impl_val {
    ($ty:ty) => {
        impl $crate::layout::Val for $ty {
            fn fields() -> &'static [$crate::layout::FieldInfo] { &[] }
            fn field_type() -> $crate::layout::FieldType {
                $crate::layout::FieldType::Struct(Self::fields)
            }
            fn layout_hash() -> u64 {
                $crate::layout::fnv_hash_bytes(
                    $crate::layout::FNV_OFFSET,
                    std::any::type_name::<$ty>().as_bytes(),
                )
            }
            fn size() -> usize { std::mem::size_of::<$ty>() }
            fn align() -> usize { std::mem::align_of::<$ty>() }
        }
    };
}
