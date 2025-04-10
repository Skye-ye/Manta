use core::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};

use arch::time::get_time_duration;

use crate::{TIMER_MANAGER, Timer};

pub enum TimeLimitedTaskOutput<T> {
    TimeOut,
    Ok(T),
}

pub struct TimeLimitedTaskFuture<F: Future + Send + 'static> {
    expire: Duration,
    future: F,
    in_timermanager: bool,
}

impl<F: Future + Send + 'static> TimeLimitedTaskFuture<F> {
    pub fn new(limit: Duration, future: F) -> Self {
        Self {
            expire: get_time_duration() + limit,
            future,
            in_timermanager: false,
        }
    }
}

impl<F: Future + Send + 'static> Future for TimeLimitedTaskFuture<F> {
    type Output = TimeLimitedTaskOutput<F::Output>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = unsafe { self.get_unchecked_mut() };
        let ret = unsafe { Pin::new_unchecked(&mut this.future).poll(cx) };
        match ret {
            Poll::Pending => {
                if get_time_duration() >= this.expire {
                    log::info!("[TimeLimitedTaskFuture] time out");
                    Poll::Ready(TimeLimitedTaskOutput::TimeOut)
                } else {
                    if !this.in_timermanager {
                        TIMER_MANAGER
                            .add_timer(Timer::new_waker_timer(this.expire, cx.waker().clone()));
                        this.in_timermanager = true;
                        log::info!("[TimeLimitedTaskFuture] first add into TIME_MANAGER");
                    }
                    Poll::Pending
                }
            }
            Poll::Ready(ret) => Poll::Ready(TimeLimitedTaskOutput::Ok(ret)),
        }
    }
}

struct IdleFuture;

impl Future for IdleFuture {
    type Output = ();
    fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
        Poll::Pending
    }
}

pub async fn ksleep_s(sec: usize) {
    TimeLimitedTaskFuture::new(Duration::from_secs(sec as u64), IdleFuture).await;
}

pub async fn ksleep_ms(msec: usize) {
    TimeLimitedTaskFuture::new(Duration::from_millis(msec as u64), IdleFuture).await;
}
