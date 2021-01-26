/// Trait for (packed) collections that can be defragmented.
pub trait Defragment {
    type Updates: Sized;

    fn defragment(&mut self) -> Option<Self::Updates>;
}

#[macro_export]
macro_rules! assign_fields_local {
    ($self:ident, [$($field:ident),*]) => {
        $(
            $self.$field = $field;
        )*
    }
}

// useful whenever you need to copy only some fields from a struct
#[macro_export]
macro_rules! assign_for_fields {
    ($self:ident, $other:ident, [$($field:ident),*], $closure:expr) => {
        $(
            $self.$field = $closure($other.$field);
        )*
    }
}
