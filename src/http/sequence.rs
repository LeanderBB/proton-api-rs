use crate::http::{ClientAsync, ClientSync, Error, FromResponse, Request};
use std::fmt::Debug;
#[cfg(not(feature = "async-traits"))]
use std::future::Future;
#[cfg(not(feature = "async-traits"))]
use std::pin::Pin;

#[cfg(not(feature = "async-traits"))]
type SequenceFuture<'a, O, E> = Pin<Box<dyn Future<Output = Result<O, E>> + 'a>>;

/// Trait which can be use to link a sequence of request operations.
pub trait Sequence<'a> {
    type Output: 'a;
    type Error: From<Error> + Debug;

    fn do_sync<T: ClientSync>(self, client: &T) -> Result<Self::Output, Self::Error>;

    #[cfg(not(feature = "async-traits"))]
    fn do_async<T: ClientAsync>(
        self,
        client: &'a T,
    ) -> SequenceFuture<'a, Self::Output, Self::Error>;

    #[cfg(feature = "async-traits")]
    async fn do_async<T: ClientAsync>(self, client: &'a T) -> Result<Self::Output, Self::Error>;

    fn map<O, E, F: FnOnce(Self::Output) -> Result<O, E>>(self, f: F) -> MapSequence<Self, F>
    where
        Self: Sized,
        E: From<Self::Error> + From<Error> + Debug,
    {
        MapSequence { c: self, f }
    }

    fn state<SS, F>(self, f: F) -> StateSequence<Self, F>
    where
        Self: Sized,
        SS: Sequence<'a>,
        F: FnOnce(Self::Output) -> SS,
        <SS as Sequence<'a>>::Error: From<Self::Error> + From<Error> + Debug,
    {
        StateSequence { seq: self, f }
    }

    fn chain<SS, E, F>(self, f: F) -> SequenceChain<Self, F>
    where
        SS: Sequence<'a>,
        F: FnOnce(Self::Output) -> Result<SS, E>,
        E: From<Self::Error> + Debug,
        <SS as Sequence<'a>>::Error: From<Self::Error> + From<E> + Debug,
        Self: Sized,
    {
        SequenceChain { s: self, f }
    }
}

impl<'a, R: Request + 'a> Sequence<'a> for R
where
    <R::Response as FromResponse>::Output: 'a,
{
    type Output = <R::Response as FromResponse>::Output;
    type Error = Error;

    fn do_sync<T: ClientSync>(self, client: &T) -> Result<Self::Output, Self::Error> {
        self.exec_sync(client)
    }

    #[cfg(not(feature = "async-traits"))]
    fn do_async<T: ClientAsync>(
        self,
        client: &'a T,
    ) -> Pin<Box<dyn Future<Output = Result<Self::Output, Self::Error>> + 'a>> {
        Box::pin(async move { self.exec_async(client).await })
    }

    #[cfg(feature = "async-traits")]
    async fn do_async<T: ClientAsync>(
        self,
        client: &'a T,
    ) -> Result<<R as Sequence<'a>>::Output, <R as Sequence<'a>>::Error> {
        self.exec_async(client).await
    }
}

#[doc(hidden)]
pub struct MapSequence<C, F> {
    c: C,
    f: F,
}

impl<'a, C, O, E, F> Sequence<'a> for MapSequence<C, F>
where
    O: 'a,
    C: Sequence<'a> + 'a,
    F: FnOnce(C::Output) -> Result<O, E> + 'a,
    E: From<Error> + Debug + From<C::Error>,
{
    type Output = O;
    type Error = E;

    fn do_sync<T: ClientSync>(self, client: &T) -> Result<Self::Output, Self::Error> {
        let v = self.c.do_sync(client)?;
        let r = (self.f)(v)?;
        Ok(r)
    }

    #[cfg(not(feature = "async-traits"))]
    fn do_async<T: ClientAsync>(
        self,
        client: &'a T,
    ) -> Pin<Box<dyn Future<Output = Result<Self::Output, Self::Error>> + 'a>> {
        Box::pin(async move {
            let v = self.c.do_async(client).await?;
            let r = (self.f)(v)?;
            Ok(r)
        })
    }

    #[cfg(feature = "async-traits")]
    async fn do_async<T: ClientAsync>(
        self,
        client: &'a T,
    ) -> Result<
        <MapSequence<C, F> as Sequence<'a>>::Output,
        <MapSequence<C, F> as Sequence<'a>>::Error,
    > {
        let v = self.c.do_async(client).await?;
        let r = (self.f)(v)?;
        Ok(r)
    }
}

#[doc(hidden)]
pub struct StateSequence<S, F> {
    seq: S,
    f: F,
}

impl<'a, S, SS, F> Sequence<'a> for StateSequence<S, F>
where
    S: Sequence<'a> + 'a,
    SS: Sequence<'a>,
    <SS as Sequence<'a>>::Error: From<<S as Sequence<'a>>::Error> + From<Error> + Debug,
    F: FnOnce(S::Output) -> SS + 'a,
{
    type Output = SS::Output;
    type Error = SS::Error;

    fn do_sync<T: ClientSync>(self, client: &T) -> Result<Self::Output, Self::Error> {
        let state = self.seq.do_sync(client)?;
        let ss = (self.f)(state);
        ss.do_sync(client)
    }

    #[cfg(not(feature = "async-traits"))]
    fn do_async<T: ClientAsync>(
        self,
        client: &'a T,
    ) -> Pin<Box<dyn Future<Output = Result<Self::Output, Self::Error>> + 'a>> {
        Box::pin(async move {
            let state = self.seq.do_async(client).await?;
            let ss = (self.f)(state);
            ss.do_async(client).await
        })
    }

    #[cfg(feature = "async-traits")]
    async fn do_async<T: ClientAsync>(
        self,
        client: &'a T,
    ) -> Result<
        <StateSequence<S, F> as Sequence<'a>>::Output,
        <StateSequence<S, F> as Sequence<'a>>::Error,
    > {
        let state = self.seq.do_async(client).await?;
        let ss = (self.f)(state);
        ss.do_async(client).await
    }
}

#[doc(hidden)]
pub struct StateProducerSequence<S, F> {
    s: S,
    f: F,
}

impl<S, F> StateProducerSequence<S, F> {
    pub fn new(s: S, f: F) -> Self {
        Self { s, f }
    }
}

impl<'a, Seq, S, F> Sequence<'a> for StateProducerSequence<S, F>
where
    Seq: Sequence<'a>,
    F: FnOnce(S) -> Seq,
{
    type Output = Seq::Output;
    type Error = Seq::Error;

    fn do_sync<T: ClientSync>(self, client: &T) -> Result<Self::Output, Self::Error> {
        let seq = (self.f)(self.s);
        seq.do_sync(client)
    }

    #[cfg(not(feature = "async-traits"))]
    fn do_async<T: ClientAsync>(
        self,
        client: &'a T,
    ) -> Pin<Box<dyn Future<Output = Result<Self::Output, Self::Error>> + 'a>> {
        let seq = (self.f)(self.s);
        seq.do_async(client)
    }

    #[cfg(feature = "async-traits")]
    async fn do_async<T: ClientAsync>(
        self,
        client: &'a T,
    ) -> Result<
        <StateProducerSequence<S, F> as Sequence<'a>>::Output,
        <StateProducerSequence<S, F> as Sequence<'a>>::Error,
    > {
        let seq = (self.f)(self.s);
        seq.do_async(client).await
    }
}

#[doc(hidden)]
pub struct SequenceChain<S, F> {
    s: S,
    f: F,
}

impl<'a, SS, S, E, F> Sequence<'a> for SequenceChain<S, F>
where
    SS: Sequence<'a>,
    S: Sequence<'a> + 'a,
    F: FnOnce(S::Output) -> Result<SS, E> + 'a,
    E: From<S::Error> + Debug,
    <SS as Sequence<'a>>::Error: From<S::Error> + From<E> + Debug,
{
    type Output = SS::Output;
    type Error = SS::Error;

    fn do_sync<T: ClientSync>(self, client: &T) -> Result<Self::Output, Self::Error> {
        let v = self.s.do_sync(client)?;
        let ss = (self.f)(v)?;
        ss.do_sync(client)
    }

    #[cfg(not(feature = "async-traits"))]
    fn do_async<T: ClientAsync>(
        self,
        client: &'a T,
    ) -> Pin<Box<dyn Future<Output = Result<Self::Output, Self::Error>> + 'a>> {
        Box::pin(async move {
            let v = self.s.do_async(client).await?;
            let ss = (self.f)(v)?;
            ss.do_async(client).await
        })
    }

    #[cfg(feature = "async-traits")]
    async fn do_async<T: ClientAsync>(
        self,
        client: &'a T,
    ) -> Result<
        <SequenceChain<S, F> as Sequence<'a>>::Output,
        <SequenceChain<S, F> as Sequence<'a>>::Error,
    > {
        let v = self.s.do_async(client).await?;
        let ss = (self.f)(v)?;
        ss.do_async(client).await
    }
}
