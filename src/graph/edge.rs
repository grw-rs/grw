use crate::id;
use crate::{Id, NR};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[derive(serde::Serialize, serde::Deserialize)]
pub enum End {
    Tgt,
    Src,
}

pub struct Anydir<U, D = U>(std::marker::PhantomData<(U, D)>);
pub struct Dir<V>(V);
pub struct Undir<V>(V);

/// Edge value wrapper for Anydir graphs: distinguishes undirected vs directed values.
/// When U = D, use Deref to transparently access the inner value.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(bound(
    serialize = "U: serde::Serialize, D: serde::Serialize",
    deserialize = "U: serde::de::DeserializeOwned, D: serde::de::DeserializeOwned",
))]
pub enum AnyVal<U, D = U> {
    Undir(U),
    Dir(D),
}


impl<V> From<V> for AnyVal<V, V> {
    fn from(v: V) -> Self { AnyVal::Undir(v) }
}

impl<U: Copy, D: Copy> Copy for AnyVal<U, D> {}

impl<U: PartialEq, D: PartialEq> PartialEq for AnyVal<U, D> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (AnyVal::Undir(a), AnyVal::Undir(b)) => a == b,
            (AnyVal::Dir(a), AnyVal::Dir(b)) => a == b,
            _ => false,
        }
    }
}

impl<U: Eq, D: Eq> Eq for AnyVal<U, D> {}

impl<U: std::hash::Hash, D: std::hash::Hash> std::hash::Hash for AnyVal<U, D> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match self {
            AnyVal::Undir(v) => { 0u8.hash(state); v.hash(state); }
            AnyVal::Dir(v) => { 1u8.hash(state); v.hash(state); }
        }
    }
}

impl<U: Default, D: Default> Default for AnyVal<U, D> {
    fn default() -> Self { AnyVal::Undir(U::default()) }
}

impl<U: crate::Val, D: crate::Val> crate::Val for AnyVal<U, D> {
    fn fields() -> &'static [crate::layout::FieldInfo] { U::fields() }
    fn methods() -> &'static [crate::layout::MethodMeta] { U::methods() }
    fn field_type() -> crate::layout::FieldType { U::field_type() }
    fn layout_hash() -> u64 {
        let mut h = U::layout_hash();
        h = crate::layout::fnv_hash_u64(h, D::layout_hash());
        h
    }
    fn size() -> usize { std::mem::size_of::<Self>() }
    fn align() -> usize { std::mem::align_of::<Self>() }
}

pub trait Edge {
    type Def: Into<(NR<id::N>, Self::Slot)>;
    type Slot: Copy + std::fmt::Debug + Ord + std::hash::Hash;
    type Val;
    type CsrStore;

    const EDGE_KIND: u8;
    const SLOT_MIN: Self::Slot;
    const SLOT_MAX: Self::Slot;
    const SLOT_COUNT: usize;

    fn edge(slot: Self::Slot, ns: (Id, Id)) -> Self::Def;
    fn reverse_slot(slot: Self::Slot) -> Self::Slot;

    fn build_csr_store(
        edges: impl Iterator<Item = (Self::Slot, Self::Val)>,
    ) -> Self::CsrStore;

    fn csr_store_val(store: &Self::CsrStore, slot: Self::Slot) -> Option<&Self::Val>;

    fn csr_any_match(store: &Self::CsrStore, pred: &dyn Fn(&Self::Val) -> bool) -> bool;

    fn csr_occupied_slots(store: &Self::CsrStore) -> smallvec::SmallVec<[Self::Slot; 3]>;
}

pub trait HasRel: Edge {
    type Rel<'a>: IntoIterator<Item = (Self::Def, &'a Self::Val)>
    where
        Self::Val: 'a;
    type RelMut<'a>
    where
        Self::Val: 'a;

    fn rel<'a>(nr: NR<id::N>, edges: impl Iterator<Item = (Self::Slot, &'a Self::Val)>) -> Self::Rel<'a>
    where
        Self::Val: 'a;
    fn rel_mut<'a>(
        nr: NR<id::N>,
        edges: impl Iterator<Item = (Self::Slot, &'a mut Self::Val)>,
    ) -> Self::RelMut<'a>
    where
        Self::Val: 'a;
}

#[diagnostic::on_unimplemented(
    message = "`{Self}` is an undirected edge type — `>>` requires a directed edge",
    label = "`{Self}` has no source→target direction",
    note = "use `^` for undirected edges, or switch to `Dir<V>` / `Anydir<V>`"
)]
pub(crate) trait Src: Edge {
    const SLOT: Self::Slot;
}

#[diagnostic::on_unimplemented(
    message = "`{Self}` is an undirected edge type — `<<` requires a directed edge",
    label = "`{Self}` has no target←source direction",
    note = "use `^` for undirected edges, or switch to `Dir<V>` / `Anydir<V>`"
)]
pub(crate) trait Tgt: Edge {
    const SLOT: Self::Slot;
}

#[diagnostic::on_unimplemented(
    message = "`{Self}` is a directed edge type — `^` requires an undirected edge",
    label = "`{Self}` has no undirected mode",
    note = "use `>>` or `<<` for directed edges, or switch to `Undir<V>` / `Anydir<V>`"
)]
pub(crate) trait Und: Edge {
    const SLOT: Self::Slot;
}

/// Maps (edge type, direction) → the value type for that slot.
/// For single-type edges (Undir<V>, Dir<V>), all slots map to V.
/// For Anydir<U, D>, undirected slots map to U, directed slots map to D.
pub trait SlotVal<Dir>: Edge {
    type SlotType;
    fn extract_slot_val(val: &Self::Val) -> Option<&Self::SlotType>;
    fn wrap_slot_val(v: Self::SlotType) -> Self::Val;
}

pub struct UndirSlot;
pub struct DirSlot;

#[inline]
pub(crate) fn normalized_val<X>(ns: [id::N; 2], norm_order: X, rev_order: X) -> (NR<id::N>, X) {
    let (ns, val) = if ns[0] <= ns[1] {
        (ns, norm_order)
    } else {
        ([ns[1], ns[0]], rev_order)
    };
    (NR(ns), val)
}

pub mod undir {
    use super::{Edge, HasRel};
    use crate::id;
    use crate::{Id, NR};

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    #[derive(serde::Serialize, serde::Deserialize)]
    pub struct Slot;

    pub const UND: Slot = Slot;

    #[derive(Debug, Clone, Copy)]
    #[derive(serde::Serialize, serde::Deserialize)]
    pub enum E<NI> {
        U(NI, NI),
    }

    impl From<E<Id>> for (NR<id::N>, Slot) {
        fn from(E::U(n1, n2): E<Id>) -> Self {
            ([id::N(n1), id::N(n2)].into(), UND)
        }
    }

    #[derive(Clone)]
    pub struct CsrStore<V>(pub(crate) V);

    impl<V> Edge for super::Undir<V> {
        type Def = E<Id>;
        type Slot = Slot;
        type Val = V;
        type CsrStore = CsrStore<V>;

        const EDGE_KIND: u8 = 0;
        const SLOT_MIN: Slot = UND;
        const SLOT_MAX: Slot = UND;
        const SLOT_COUNT: usize = 1;

        fn edge(Slot: Self::Slot, (n1, n2): (Id, Id)) -> Self::Def {
            E::U(n1, n2)
        }

        fn reverse_slot(_slot: Self::Slot) -> Self::Slot {
            Slot
        }

        fn build_csr_store(
            edges: impl Iterator<Item = (Self::Slot, V)>,
        ) -> CsrStore<V> {
            let mut val = None;
            for (_, v) in edges {
                val = Some(v);
            }
            CsrStore(val.unwrap())
        }

        fn csr_store_val(store: &CsrStore<V>, _slot: Slot) -> Option<&V> {
            Some(&store.0)
        }

        fn csr_any_match(store: &CsrStore<V>, pred: &dyn Fn(&V) -> bool) -> bool {
            pred(&store.0)
        }

        fn csr_occupied_slots(_store: &CsrStore<V>) -> smallvec::SmallVec<[Slot; 3]> {
            smallvec::smallvec![UND]
        }
    }

    impl<V> super::SlotVal<super::UndirSlot> for super::Undir<V> {
        type SlotType = V;
        fn extract_slot_val(val: &V) -> Option<&V> { Some(val) }
        fn wrap_slot_val(v: V) -> V { v }
    }

    impl<V> super::Und for super::Undir<V> {
        const SLOT: Slot = UND;
    }

    pub struct Rel<'a, V> {
        nr: NR<id::N>,
        pub undir: Option<&'a V>,
    }

    pub struct RelMut<'a, V> {
        nr: NR<id::N>,
        pub undir: Option<&'a mut V>,
    }

    impl<V> HasRel for super::Undir<V> {
        type Rel<'a>
            = Rel<'a, V>
        where
            V: 'a;
        type RelMut<'a>
            = RelMut<'a, V>
        where
            V: 'a;

        fn rel<'a>(nr: NR<id::N>, edges: impl Iterator<Item = (Slot, &'a V)>) -> Rel<'a, V>
        where
            V: 'a,
        {
            let mut undir = None;
            for (_, v) in edges {
                undir = Some(v);
            }
            Rel { nr, undir }
        }

        fn rel_mut<'a>(nr: NR<id::N>, edges: impl Iterator<Item = (Slot, &'a mut V)>) -> RelMut<'a, V>
        where
            V: 'a,
        {
            let mut undir = None;
            for (_, v) in edges {
                undir = Some(v);
            }
            RelMut { nr, undir }
        }
    }

    impl<'a, V> IntoIterator for Rel<'a, V> {
        type Item = (E<Id>, &'a V);
        type IntoIter = std::option::IntoIter<Self::Item>;

        fn into_iter(self) -> Self::IntoIter {
            let (n1, n2) = (**self.nr.n1(), **self.nr.n2());
            self.undir.map(|v| (E::U(n1, n2), v)).into_iter()
        }
    }

    impl<'a, V> IntoIterator for RelMut<'a, V> {
        type Item = (E<Id>, &'a mut V);
        type IntoIter = std::option::IntoIter<Self::Item>;

        fn into_iter(self) -> Self::IntoIter {
            let (n1, n2) = (**self.nr.n1(), **self.nr.n2());
            self.undir.map(|v| (E::U(n1, n2), v)).into_iter()
        }
    }
}

pub mod dir {
    use super::{Edge, End, HasRel};
    use crate::id;
    use crate::{Id, NR};

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    #[derive(serde::Serialize, serde::Deserialize)]
    pub struct Slot(pub End);

    pub const TGT: Slot = Slot(End::Tgt);
    pub const SRC: Slot = Slot(End::Src);

    #[derive(Debug, Clone, Copy)]
    #[derive(serde::Serialize, serde::Deserialize)]
    pub enum E<NI> {
        D(NI, NI),
    }

    impl From<E<Id>> for (NR<id::N>, Slot) {
        fn from(E::D(n1, n2): E<Id>) -> Self {
            super::normalized_val([id::N(n1), id::N(n2)], SRC, TGT)
        }
    }

    #[derive(Clone)]
    pub struct CsrStore<V>(pub(crate) Option<V>, pub(crate) Option<V>);

    impl<V> Edge for super::Dir<V> {
        type Def = E<Id>;
        type Slot = Slot;
        type Val = V;
        type CsrStore = CsrStore<V>;

        const EDGE_KIND: u8 = 1;
        const SLOT_MIN: Slot = TGT;
        const SLOT_MAX: Slot = SRC;
        const SLOT_COUNT: usize = 2;

        fn edge(slot: Self::Slot, (n1, n2): (Id, Id)) -> Self::Def {
            match slot {
                Slot(End::Src) => E::D(n1, n2),
                Slot(End::Tgt) => E::D(n2, n1),
            }
        }

        fn reverse_slot(slot: Self::Slot) -> Self::Slot {
            match slot {
                Slot(End::Src) => TGT,
                Slot(End::Tgt) => SRC,
            }
        }

        fn build_csr_store(
            edges: impl Iterator<Item = (Self::Slot, V)>,
        ) -> CsrStore<V> {
            let mut tgt = None;
            let mut src = None;
            for (slot, v) in edges {
                match slot {
                    Slot(End::Tgt) => tgt = Some(v),
                    Slot(End::Src) => src = Some(v),
                }
            }
            CsrStore(tgt, src)
        }

        fn csr_store_val(store: &CsrStore<V>, slot: Slot) -> Option<&V> {
            match slot.0 {
                End::Tgt => store.0.as_ref(),
                End::Src => store.1.as_ref(),
            }
        }

        fn csr_any_match(store: &CsrStore<V>, pred: &dyn Fn(&V) -> bool) -> bool {
            store.0.as_ref().is_some_and(|v| pred(v))
                || store.1.as_ref().is_some_and(|v| pred(v))
        }

        fn csr_occupied_slots(store: &CsrStore<V>) -> smallvec::SmallVec<[Slot; 3]> {
            let mut out = smallvec::SmallVec::new();
            if store.0.is_some() { out.push(TGT); }
            if store.1.is_some() { out.push(SRC); }
            out
        }
    }

    impl<V> super::SlotVal<super::DirSlot> for super::Dir<V> {
        type SlotType = V;
        fn extract_slot_val(val: &V) -> Option<&V> { Some(val) }
        fn wrap_slot_val(v: V) -> V { v }
    }

    impl<V> super::Src for super::Dir<V> {
        const SLOT: Slot = SRC;
    }

    impl<V> super::Tgt for super::Dir<V> {
        const SLOT: Slot = TGT;
    }

    pub struct Rel<'a, V> {
        nr: NR<id::N>,
        pub out: Option<&'a V>,
        pub inc: Option<&'a V>,
    }

    pub struct RelMut<'a, V> {
        nr: NR<id::N>,
        pub out: Option<&'a mut V>,
        pub inc: Option<&'a mut V>,
    }

    impl<V> HasRel for super::Dir<V> {
        type Rel<'a>
            = Rel<'a, V>
        where
            V: 'a;
        type RelMut<'a>
            = RelMut<'a, V>
        where
            V: 'a;

        fn rel<'a>(nr: NR<id::N>, edges: impl Iterator<Item = (Slot, &'a V)>) -> Rel<'a, V>
        where
            V: 'a,
        {
            let mut out = None;
            let mut inc = None;
            for (slot, v) in edges {
                match slot {
                    Slot(End::Src) => out = Some(v),
                    Slot(End::Tgt) => inc = Some(v),
                }
            }
            Rel { nr, out, inc }
        }

        fn rel_mut<'a>(nr: NR<id::N>, edges: impl Iterator<Item = (Slot, &'a mut V)>) -> RelMut<'a, V>
        where
            V: 'a,
        {
            let mut out = None;
            let mut inc = None;
            for (slot, v) in edges {
                match slot {
                    Slot(End::Src) => out = Some(v),
                    Slot(End::Tgt) => inc = Some(v),
                }
            }
            RelMut { nr, out, inc }
        }
    }

    impl<'a, V> IntoIterator for Rel<'a, V> {
        type Item = (E<Id>, &'a V);
        type IntoIter = smallvec::IntoIter<[Self::Item; 2]>;

        fn into_iter(self) -> Self::IntoIter {
            let (n1, n2) = (**self.nr.n1(), **self.nr.n2());
            let mut items = smallvec::SmallVec::<[_; 2]>::new();
            if let Some(v) = self.out {
                items.push((E::D(n1, n2), v));
            }
            if let Some(v) = self.inc {
                items.push((E::D(n2, n1), v));
            }
            items.into_iter()
        }
    }

    impl<'a, V> IntoIterator for RelMut<'a, V> {
        type Item = (E<Id>, &'a mut V);
        type IntoIter = smallvec::IntoIter<[Self::Item; 2]>;

        fn into_iter(self) -> Self::IntoIter {
            let (n1, n2) = (**self.nr.n1(), **self.nr.n2());
            let mut items = smallvec::SmallVec::<[_; 2]>::new();
            if let Some(v) = self.out {
                items.push((E::D(n1, n2), v));
            }
            if let Some(v) = self.inc {
                items.push((E::D(n2, n1), v));
            }
            items.into_iter()
        }
    }
}

pub mod anydir {
    use super::{Edge, End, HasRel};
    use crate::id;
    use crate::{Id, NR};

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    #[derive(serde::Serialize, serde::Deserialize)]
    pub enum Slot {
        Dir(End),
        Undir,
    }

    pub const TGT: Slot = Slot::Dir(End::Tgt);
    pub const SRC: Slot = Slot::Dir(End::Src);
    pub const UND: Slot = Slot::Undir;

    impl From<End> for Slot {
        fn from(end: End) -> Self {
            Slot::Dir(end)
        }
    }

    impl From<super::undir::Slot> for Slot {
        fn from(_: super::undir::Slot) -> Self {
            Slot::Undir
        }
    }

    #[derive(Debug, Clone, Copy)]
    #[derive(serde::Serialize, serde::Deserialize)]
    pub enum E<NI> {
        U(NI, NI),
        D(NI, NI),
    }

    impl From<E<Id>> for (NR<id::N>, Slot) {
        fn from(value: E<Id>) -> Self {
            match value {
                E::U(n1, n2) => ([id::N(n1), id::N(n2)].into(), UND),
                E::D(n1, n2) => super::normalized_val([id::N(n1), id::N(n2)], SRC, TGT),
            }
        }
    }

    #[derive(Clone)]
    pub struct CsrStore<U, D>(
        pub(crate) Option<super::AnyVal<U, D>>,
        pub(crate) Option<super::AnyVal<U, D>>,
        pub(crate) Option<super::AnyVal<U, D>>,
    );

    impl<U, D> Edge for super::Anydir<U, D> {
        type Def = E<Id>;
        type Slot = Slot;
        type Val = super::AnyVal<U, D>;
        type CsrStore = CsrStore<U, D>;

        const EDGE_KIND: u8 = 2;
        const SLOT_MIN: Slot = TGT;
        const SLOT_MAX: Slot = UND;
        const SLOT_COUNT: usize = 3;

        fn edge(slot: Self::Slot, (n1, n2): (Id, Id)) -> Self::Def {
            match slot {
                Slot::Dir(End::Src) => E::D(n1, n2),
                Slot::Dir(End::Tgt) => E::D(n2, n1),
                Slot::Undir => E::U(n1, n2),
            }
        }

        fn reverse_slot(slot: Self::Slot) -> Self::Slot {
            match slot {
                Slot::Dir(End::Src) => TGT,
                Slot::Dir(End::Tgt) => SRC,
                Slot::Undir => UND,
            }
        }

        fn build_csr_store(
            edges: impl Iterator<Item = (Self::Slot, super::AnyVal<U, D>)>,
        ) -> CsrStore<U, D> {
            let mut tgt = None;
            let mut src = None;
            let mut und = None;
            for (slot, v) in edges {
                match slot {
                    Slot::Dir(End::Tgt) => tgt = Some(v),
                    Slot::Dir(End::Src) => src = Some(v),
                    Slot::Undir => und = Some(v),
                }
            }
            CsrStore(tgt, src, und)
        }

        fn csr_store_val(store: &CsrStore<U, D>, slot: Slot) -> Option<&super::AnyVal<U, D>> {
            match slot {
                Slot::Dir(End::Tgt) => store.0.as_ref(),
                Slot::Dir(End::Src) => store.1.as_ref(),
                Slot::Undir => store.2.as_ref(),
            }
        }

        fn csr_any_match(store: &CsrStore<U, D>, pred: &dyn Fn(&super::AnyVal<U, D>) -> bool) -> bool {
            store.0.as_ref().is_some_and(|v| pred(v))
                || store.1.as_ref().is_some_and(|v| pred(v))
                || store.2.as_ref().is_some_and(|v| pred(v))
        }

        fn csr_occupied_slots(store: &CsrStore<U, D>) -> smallvec::SmallVec<[Slot; 3]> {
            let mut out = smallvec::SmallVec::new();
            if store.0.is_some() { out.push(TGT); }
            if store.1.is_some() { out.push(SRC); }
            if store.2.is_some() { out.push(UND); }
            out
        }
    }

    impl<U, D> super::SlotVal<super::UndirSlot> for super::Anydir<U, D> {
        type SlotType = U;
        fn extract_slot_val(val: &super::AnyVal<U, D>) -> Option<&U> {
            match val { super::AnyVal::Undir(u) => Some(u), _ => None }
        }
        fn wrap_slot_val(v: U) -> super::AnyVal<U, D> { super::AnyVal::Undir(v) }
    }

    impl<U, D> super::SlotVal<super::DirSlot> for super::Anydir<U, D> {
        type SlotType = D;
        fn extract_slot_val(val: &super::AnyVal<U, D>) -> Option<&D> {
            match val { super::AnyVal::Dir(d) => Some(d), _ => None }
        }
        fn wrap_slot_val(v: D) -> super::AnyVal<U, D> { super::AnyVal::Dir(v) }
    }

    impl<U, D> super::Src for super::Anydir<U, D> {
        const SLOT: Slot = SRC;
    }

    impl<U, D> super::Tgt for super::Anydir<U, D> {
        const SLOT: Slot = TGT;
    }

    impl<U, D> super::Und for super::Anydir<U, D> {
        const SLOT: Slot = UND;
    }

    pub struct Rel<'a, U, D> {
        nr: NR<id::N>,
        pub out: Option<&'a super::AnyVal<U, D>>,
        pub inc: Option<&'a super::AnyVal<U, D>>,
        pub undir: Option<&'a super::AnyVal<U, D>>,
    }

    pub struct RelMut<'a, U, D> {
        nr: NR<id::N>,
        pub out: Option<&'a mut super::AnyVal<U, D>>,
        pub inc: Option<&'a mut super::AnyVal<U, D>>,
        pub undir: Option<&'a mut super::AnyVal<U, D>>,
    }

    impl<U: 'static, D: 'static> HasRel for super::Anydir<U, D> {
        type Rel<'a>
            = Rel<'a, U, D>;
        type RelMut<'a>
            = RelMut<'a, U, D>;

        fn rel<'a>(nr: NR<id::N>, edges: impl Iterator<Item = (Slot, &'a super::AnyVal<U, D>)>) -> Rel<'a, U, D>
        where
            super::AnyVal<U, D>: 'a,
        {
            let mut out = None;
            let mut inc = None;
            let mut undir = None;
            for (slot, v) in edges {
                match slot {
                    Slot::Dir(End::Src) => out = Some(v),
                    Slot::Dir(End::Tgt) => inc = Some(v),
                    Slot::Undir => undir = Some(v),
                }
            }
            Rel { nr, out, inc, undir }
        }

        fn rel_mut<'a>(nr: NR<id::N>, edges: impl Iterator<Item = (Slot, &'a mut super::AnyVal<U, D>)>) -> RelMut<'a, U, D>
        where
            super::AnyVal<U, D>: 'a,
        {
            let mut out = None;
            let mut inc = None;
            let mut undir = None;
            for (slot, v) in edges {
                match slot {
                    Slot::Dir(End::Src) => out = Some(v),
                    Slot::Dir(End::Tgt) => inc = Some(v),
                    Slot::Undir => undir = Some(v),
                }
            }
            RelMut { nr, out, inc, undir }
        }
    }

    impl<'a, U, D> IntoIterator for Rel<'a, U, D> {
        type Item = (E<Id>, &'a super::AnyVal<U, D>);
        type IntoIter = smallvec::IntoIter<[Self::Item; 3]>;

        fn into_iter(self) -> Self::IntoIter {
            let (n1, n2) = (**self.nr.n1(), **self.nr.n2());
            let mut items = smallvec::SmallVec::<[_; 3]>::new();
            if let Some(v) = self.out {
                items.push((E::D(n1, n2), v));
            }
            if let Some(v) = self.inc {
                items.push((E::D(n2, n1), v));
            }
            if let Some(v) = self.undir {
                items.push((E::U(n1, n2), v));
            }
            items.into_iter()
        }
    }

    impl<'a, U, D> IntoIterator for RelMut<'a, U, D> {
        type Item = (E<Id>, &'a mut super::AnyVal<U, D>);
        type IntoIter = smallvec::IntoIter<[Self::Item; 3]>;

        fn into_iter(self) -> Self::IntoIter {
            let (n1, n2) = (**self.nr.n1(), **self.nr.n2());
            let mut items = smallvec::SmallVec::<[_; 3]>::new();
            if let Some(v) = self.out {
                items.push((E::D(n1, n2), v));
            }
            if let Some(v) = self.inc {
                items.push((E::D(n2, n1), v));
            }
            if let Some(v) = self.undir {
                items.push((E::U(n1, n2), v));
            }
            items.into_iter()
        }
    }
}

pub struct Batch<E: Edge>(pub(crate) Vec<(E::Def, E::Val)>);

macro_rules! impl_batch {
    ($name:ident, $mod:ident) => {
        impl From<Vec<crate::edge::$mod::E<Id>>> for Batch<$name<()>> {
            fn from(es: Vec<crate::edge::$mod::E<Id>>) -> Self {
                Batch(es.into_iter().map(|e| (e, ())).collect())
            }
        }
        impl<EV> From<Vec<(crate::edge::$mod::E<Id>, EV)>> for Batch<$name<EV>> {
            fn from(evs: Vec<(crate::edge::$mod::E<Id>, EV)>) -> Self {
                Batch(evs)
            }
        }
    };
}

impl_batch!(Undir, undir);
impl_batch!(Dir, dir);

impl From<Vec<crate::edge::anydir::E<Id>>> for Batch<Anydir<()>> {
    fn from(es: Vec<crate::edge::anydir::E<Id>>) -> Self {
        Batch(es.into_iter().map(|e| (e, AnyVal::Undir(()))).collect())
    }
}
impl<EV> From<Vec<(crate::edge::anydir::E<Id>, EV)>> for Batch<Anydir<EV>> {
    fn from(evs: Vec<(crate::edge::anydir::E<Id>, EV)>) -> Self {
        Batch(evs.into_iter().map(|(e, v)| (e, AnyVal::Undir(v))).collect())
    }
}
