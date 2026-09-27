use super::{Composite, Included, KindOf, Kinded, Removed};

pub trait PartOf<T>: Kinded {
    fn kind() -> <Self as Kinded>::Kind;

    fn project(&self) -> Option<&T>;

    fn inject(part: T) -> Self;
}

pub trait Has<T>: Composite {
    fn part_kind() -> KindOf<Self>;

    fn typed(&self) -> Option<&T>;

    fn with_typed(&self, part: T) -> Result<Included<Self>, Self::IncludeRefused>;

    fn without_typed(&self) -> Result<Removed<Self>, Self::ExcludeRefused>;
}

impl<T, C> Has<T> for C
where
    C: Composite,
    C::Part: PartOf<T>,
{
    fn part_kind() -> KindOf<Self> {
        <C::Part as PartOf<T>>::kind()
    }

    fn typed(&self) -> Option<&T> {
        let kind = <C::Part as PartOf<T>>::kind();
        self.part(&kind).and_then(PartOf::<T>::project)
    }

    fn with_typed(&self, part: T) -> Result<Included<Self>, Self::IncludeRefused> {
        self.with_part(<C::Part as PartOf<T>>::inject(part))
    }

    fn without_typed(&self) -> Result<Removed<Self>, Self::ExcludeRefused> {
        self.without_part(&<C::Part as PartOf<T>>::kind())
    }
}

pub trait Typed: Composite {
    fn get<T>(&self) -> Option<&T>
    where
        Self: Has<T>;

    fn include<T>(&self, part: T) -> Result<Included<Self>, Self::IncludeRefused>
    where
        Self: Has<T>;

    fn exclude<T>(&self) -> Result<Removed<Self>, Self::ExcludeRefused>
    where
        Self: Has<T>;
}

impl<C: Composite> Typed for C {
    fn get<T>(&self) -> Option<&T>
    where
        Self: Has<T>,
    {
        Has::<T>::typed(self)
    }

    fn include<T>(&self, part: T) -> Result<Included<Self>, Self::IncludeRefused>
    where
        Self: Has<T>,
    {
        Has::<T>::with_typed(self, part)
    }

    fn exclude<T>(&self) -> Result<Removed<Self>, Self::ExcludeRefused>
    where
        Self: Has<T>,
    {
        Has::<T>::without_typed(self)
    }
}
