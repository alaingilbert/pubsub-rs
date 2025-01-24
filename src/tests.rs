#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use crate::{Pubsub, PubsubError};

    #[tokio::test]
    async fn test_pubsub() {
        let ps = Pubsub::new();
        let s1 = ps.subscribe(vec!["a", "b"]).await;
        ps.publish("a", "msg1".to_owned()).await;
        ps.publish("b", "msg2".to_owned()).await;
        let (t1, m1) = s1.recv().await.unwrap();
        assert_eq!(t1, "a".to_owned());
        assert_eq!(m1, "msg1".to_owned());
        let (t2, m2) = s1.recv().await.unwrap();
        assert_eq!(t2, "b".to_owned());
        assert_eq!(m2, "msg2".to_owned());
    }

    #[tokio::test]
    async fn test_pubsub_thread() {
        let ps = Pubsub::new();
        let s1 = ps.subscribe(vec!["a", "b"]).await;
        ps.publish("a", "msg1".to_owned()).await;
        // ensure Subscriber can be moved to different thread
        tokio::spawn(async move {
            s1.recv().await.unwrap();
        });
        assert_eq!(1, 1);
    }

    #[tokio::test]
    async fn test_pubsub_dropped1() {
        let ps: Pubsub<&str, String> = Pubsub::new();
        let s1 = ps.subscribe(vec!["a", "b"]).await;
        drop(ps);
        let (tx, rx) = tokio::sync::oneshot::channel();
        tokio::spawn(async move {
            let Err(e) = s1.recv().await else {
                panic!("should get error");
            };
            assert_eq!(e, PubsubError);
            tx.send(()).unwrap();
        });
        rx.await.unwrap();
        assert_eq!(1, 1);
    }

    #[tokio::test]
    async fn test_pubsub_dropped2() {
        let ps: Pubsub<&str, String> = Pubsub::new();
        let s1 = ps.subscribe(vec!["a", "b"]).await;
        let (tx, rx) = tokio::sync::oneshot::channel();
        tokio::spawn(async move {
            tokio::spawn(async move {
                drop(ps);
            });
            let Err(e) = s1.recv().await else {
                panic!("should get error");
            };
            assert_eq!(e, PubsubError);
            tx.send(()).unwrap();
        });
        rx.await.unwrap();
        assert_eq!(1, 1);
    }

    #[tokio::test]
    async fn test_pubsub_dropped3() {
        let ps: Pubsub<&str, String> = Pubsub::new();
        let ps2 = ps.clone();
        let s1 = ps.subscribe(vec!["a", "b"]).await;
        assert_eq!(Arc::strong_count(&ps.inner), 2);
        let (tx, rx) = tokio::sync::oneshot::channel();
        tokio::spawn(async move {
            tokio::spawn(async move {
                drop(ps);
                tokio::spawn(async move {
                    ps2.publish("a", "test".to_owned()).await;
                });
            });
            let Ok((t, p)) = s1.recv().await else {
                panic!("should not get error");
            };
            assert_eq!(t, "a");
            assert_eq!(p, "test".to_owned());
            tx.send(()).unwrap();
        });
        rx.await.unwrap();
        assert_eq!(1, 1);
    }

    #[tokio::test]
    async fn test_pubsub_pubsub_thread() {
        let ps = Pubsub::new();
        let ps1 = ps.clone();
        let (tx, rx) = tokio::sync::oneshot::channel();
        let s1 = ps.subscribe(vec!["a", "b"]).await;
        tokio::spawn(async move {
            ps1.publish("a", "msg1".to_owned()).await;
            tx.send(()).unwrap();
        });
        rx.await.unwrap();
        let (t1, m1) = s1.recv().await.unwrap();
        assert_eq!(t1, "a".to_owned());
        assert_eq!(m1, "msg1".to_owned());
    }

    #[tokio::test]
    async fn test_pubsub_subscriber_drop_cleanup() {
        let ps: Pubsub<&str, String> = Pubsub::new();
        let s1 = ps.subscribe(vec!["a", "b"]).await;
        let s2 = ps.subscribe(vec!["a", "b"]).await;
        assert_eq!(ps.inner.m.get("a").unwrap().len(), 2);
        drop(s1);
        assert_eq!(ps.inner.m.get("a").unwrap().len(), 1);
        drop(s2);
        assert_eq!(ps.inner.m.get("a").unwrap().len(), 0);
    }
}
