use super::*;
use crate::actor::watchdog::WatchdogMsg::Register;
use crate::actor::watchdog::{Watchdog, WatchdogMsg};
use crate::bot::SlackHandlerResponse::Handled;
use ractor::*;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tracing::info;

#[concurrency::test]
#[tracing_test::traced_test]
async fn test_supervision_panic_in_post_startup() {
    struct Child;
    struct Supervisor {
        flag: Arc<AtomicU64>,
    }

    #[async_trait::async_trait]
    impl Actor for Child {
        type Msg = ();
        type State = ();
        type Arguments = ();
        async fn pre_start(
            &self,
            _this_actor: ActorRef<Self::Msg>,
            _: (),
        ) -> Result<Self::State, ActorProcessingErr> {
            Ok(())
        }
        async fn post_start(
            &self,
            _this_actor: ActorRef<Self::Msg>,
            _state: &mut Self::State,
        ) -> Result<(), ActorProcessingErr> {
            panic!("Boom");
        }
    }

    #[async_trait::async_trait]
    impl Actor for Supervisor {
        type Msg = ();
        type State = ();
        type Arguments = ();
        async fn pre_start(
            &self,
            _this_actor: ActorRef<Self::Msg>,
            _: (),
        ) -> Result<Self::State, ActorProcessingErr> {
            Ok(())
        }
        async fn handle(
            &self,
            _this_actor: ActorRef<Self::Msg>,
            _message: Self::Msg,
            _state: &mut Self::State,
        ) -> Result<(), ActorProcessingErr> {
            Ok(())
        }

        async fn handle_supervisor_evt(
            &self,
            this_actor: ActorRef<Self::Msg>,
            message: SupervisionEvent,
            _state: &mut Self::State,
        ) -> Result<(), ActorProcessingErr> {
            println!("Supervisor event received {message:?}");

            // check that the panic was captured
            if let SupervisionEvent::ActorFailed(dead_actor, _panic_msg) = message {
                self.flag.store(dead_actor.get_id().pid(), Ordering::SeqCst);
                this_actor.stop(None);
            }
            Ok(())
        }
    }

    let flag = Arc::new(AtomicU64::new(0));

    let (supervisor_ref, s_handle) = Actor::spawn(None, Supervisor { flag: flag.clone() }, ())
        .await
        .expect("Supervisor panicked on startup");

    let (child_ref, c_handle) = supervisor_ref
        .spawn_linked(None, Child, ())
        .await
        .expect("Child panicked on startup");

    let maybe_sup = child_ref.try_get_supervisor();
    assert!(maybe_sup.is_some());
    assert_eq!(maybe_sup.map(|a| a.get_id()), Some(supervisor_ref.get_id()));

    let (_, _) = tokio::join!(s_handle, c_handle);

    assert_eq!(child_ref.get_id().pid(), flag.load(Ordering::SeqCst));

    // supervisor relationship cleaned up correctly
    assert_eq!(0, supervisor_ref.get_children().len());
}

#[concurrency::test]
#[tracing_test::traced_test]
async fn test_foo() {
    static HANDLE: AtomicBool = AtomicBool::new(false);

    struct MyActor;

    #[async_trait::async_trait]
    impl Actor for MyActor {
        type Msg = String;
        type State = ActorRef<WatchdogMsg>;
        type Arguments = ActorRef<WatchdogMsg>;

        async fn pre_start(
            &self,
            myself: ActorRef<Self::Msg>,
            watchdog: Self::Arguments,
        ) -> Result<Self::State, ActorProcessingErr> {
            cast!(
                watchdog.clone(),
                Register(myself.get_cell(), Duration::from_millis(500))
            )?;

            myself.send_after(Duration::from_millis(400), || "hello".to_string());

            Ok(watchdog)
        }

        async fn handle(
            &self,
            myself: ActorRef<Self::Msg>,
            msg: Self::Msg,
            state: &mut Self::State,
        ) -> Result<(), ActorProcessingErr> {
            info!("handle() msg={}", msg);
            HANDLE.store(true, Ordering::SeqCst);
            cast!(
                state,
                Register(myself.get_cell(), Duration::from_millis(500))
            )
            .map_err(|e| ActorProcessingErr::from(e))
        }
    }

    info!("starting");
    let (watchdog, watchdog_handle) = Actor::spawn(None, Watchdog, ()).await.unwrap();
    info!("watchdog started");

    info!("starting my_actor");
    let (my_actor, my_actor_handle) = Actor::spawn(None, MyActor, watchdog.clone()).await.unwrap();
    info!("my_actor started");

    tokio::time::sleep(Duration::from_millis(100)).await;

    assert_eq!(false, HANDLE.load(Ordering::SeqCst));
    assert_eq!(ActorStatus::Running, my_actor.get_status());

    tokio::time::sleep(Duration::from_millis(3000)).await;

    assert_eq!(true, HANDLE.load(Ordering::SeqCst));
    assert_eq!(ActorStatus::Stopped, my_actor.get_status());

    my_actor_handle.await.unwrap();

    watchdog.stop(None);

    watchdog_handle.await.unwrap();
}
