use std::any::Any;
use std::fmt::Debug;

trait ErasedKind: Debug + Send + Sync {
    fn as_any(&self) -> &dyn Any;

    fn same(&self, other: &dyn ErasedKind) -> bool;
}

impl<K: PartialEq + Debug + Send + Sync + 'static> ErasedKind for K {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn same(&self, other: &dyn ErasedKind) -> bool {
        match other.as_any().downcast_ref::<K>() {
            Some(theirs) => theirs == self,
            None => false,
        }
    }
}

pub struct PartKind(Box<dyn ErasedKind>);

impl PartKind {
    pub(crate) fn new<K: PartialEq + Debug + Send + Sync + 'static>(kind: K) -> Self {
        PartKind(Box::new(kind))
    }

    pub(crate) fn same(&self, other: &PartKind) -> bool {
        self.0.same(other.0.as_ref())
    }

    pub fn of<K: 'static>(&self) -> Option<&K> {
        self.0.as_any().downcast_ref()
    }
}

impl Debug for PartKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(&self.0, f)
    }
}
