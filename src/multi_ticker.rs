use std::collections::HashMap;

use std::hash::Hash;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Instant;

use futures::Stream;
use futures_ticker::Ticker;
use futures_util::stream::select_all;
use futures_util::StreamExt;

struct MultiTicker<T: Copy + Hash + Eq + Unpin + 'static> {
    tickers: HashMap<T, Ticker>,
}

struct KeyedStream<T, S>(T, S);

impl<T, S> Stream for KeyedStream<T, S>
where
    T: Copy + Unpin,
    S: Stream + Unpin,
{
    type Item = (T, S::Item);

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let KeyedStream(key, stream) = self.get_mut();
        match stream.poll_next_unpin(cx) {
            Poll::Ready(Some(item)) => Poll::Ready(Some((*key, item))),
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (1, None)
    }
}

impl<T: Copy + Hash + Eq + Unpin + 'static> MultiTicker<T> {}

impl<T: Copy + Hash + Eq + Unpin + 'static> Stream for MultiTicker<T> {
    type Item = (T, Instant);

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        select_all(
            self.get_mut()
                .tickers
                .iter_mut()
                .map(|(k, v)| KeyedStream(*k, v)),
        )
        .poll_next_unpin(cx)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.tickers.len(), None)
    }
}

#[cfg(test)]
mod test {
    use std::time::Duration;

    use futures::executor::block_on;

    use super::*;

    #[test]
    fn test_multi_ticker() {
        #[derive(Clone, Copy, Eq, Debug, Hash, PartialEq)]
        enum TickerId {
            A,
            B,
            C,
        }

        let mut tickers = HashMap::new();
        tickers.insert(TickerId::A, Ticker::new(Duration::from_millis(1000)));
        tickers.insert(TickerId::B, Ticker::new(Duration::from_millis(2100)));
        tickers.insert(TickerId::C, Ticker::new(Duration::from_millis(3200)));

        let mut multi_ticker = MultiTicker { tickers };

        let mut ticks = vec![];
        let mut multi_ticker = Pin::new(&mut multi_ticker);
        for _ in 0..5 {
            ticks.push(block_on(multi_ticker.next()).unwrap());
        }

        println!("{:?}", ticks);

        assert_eq!(
            ticks,
            vec![
                (TickerId::A, ticks[0].1), // 1000
                (TickerId::A, ticks[1].1), // 2000
                (TickerId::B, ticks[2].1), // 2100
                (TickerId::A, ticks[3].1), // 3000
                (TickerId::C, ticks[4].1), // 3200
            ]
        );
    }
}
