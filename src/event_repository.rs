/// Implements the `EventRepository` trait for a SQLite database.

use async_trait::async_trait;
use cqrs_es::{
    persist::{
        PersistedEventRepository, PersistenceError, ReplayStream, SerializedEvent,
        SerializedSnapshot,
    },
    Aggregate,
};
use futures::TryStreamExt;
use serde_json::Value;
use sqlx::{Pool, Sqlite, Transaction};

use crate::SqliteAggregateError;

/// Stream Channel size, hardcoded for now.
const STREAM_CHANNEL_SIZE: usize = 100;

pub struct SqliteEventRepository {
    pool: Pool<Sqlite>,
}

impl SqliteEventRepository {
    pub fn new(pool: Pool<Sqlite>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl PersistedEventRepository for SqliteEventRepository {
    async fn get_events<A: Aggregate>(
        &self,
        aggregate_id: &str,
    ) -> Result<Vec<SerializedEvent>, PersistenceError> {
        let aggregate_type = A::aggregate_type();
        let rows = sqlx::query!(
            r#"
            SELECT
                aggregate_type,
                aggregate_id,
                sequence,
                event_type,
                event_version,
                payload,
                metadata
            FROM
                events
            WHERE
                aggregate_type = $1 AND
                aggregate_id = $2
            ORDER BY
                sequence
            "#,
            aggregate_type,
            aggregate_id
        )
        .fetch_all(&self.pool)
        .await
        .map_err(SqliteAggregateError::from)?;

        let mut result: Vec<SerializedEvent> = Default::default();
        for row in rows {
            result.push(SerializedEvent {
                aggregate_id: aggregate_id.to_string(),
                aggregate_type: row.aggregate_type,
                event_version: row.event_version,
                payload: serde_json::Value::String(row.payload),
                metadata: serde_json::Value::String(row.metadata),
                sequence: row.sequence as usize,
                event_type: row.event_type,
            });
        }

        Ok(result)
    }

    async fn get_last_events<A: Aggregate>(
        &self,
        aggregate_id: &str,
        last_sequence: usize,
    ) -> Result<Vec<SerializedEvent>, PersistenceError> {
        let aggregate_type = A::aggregate_type();
        let last_sequence = last_sequence as i64;

        let rows = sqlx::query!(
            r#"
            SELECT
                aggregate_type,
                aggregate_id,
                sequence,
                event_type,
                event_version,
                payload,
                metadata
            FROM
              events
            WHERE
              aggregate_type = $1 AND aggregate_id = $2 AND sequence > $3
            ORDER BY sequence"#,
            aggregate_type,
            aggregate_id,
            last_sequence,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(SqliteAggregateError::from)?;

        let mut result: Vec<SerializedEvent> = Default::default();
        for row in rows {
            result.push(SerializedEvent {
                aggregate_id: aggregate_id.to_string(),
                aggregate_type: row.aggregate_type,
                event_version: row.event_version,
                payload: serde_json::Value::String(row.payload),
                metadata: serde_json::Value::String(row.metadata),
                sequence: row.sequence as usize,
                event_type: row.event_type,
            });
        }

        Ok(result)
    }

    async fn get_snapshot<A: Aggregate>(
        &self,
        aggregate_id: &str,
    ) -> Result<Option<SerializedSnapshot>, PersistenceError> {
        todo!()
    }

    async fn persist<A: Aggregate>(
        &self,
        events: &[SerializedEvent],
        snapshot_update: Option<(String, Value, usize)>,
    ) -> Result<(), PersistenceError> {
        let mut tx: Transaction<'_, Sqlite> = sqlx::Acquire::begin(&self.pool)
            .await
            .map_err(SqliteAggregateError::from)?;
        let res = match snapshot_update {
            None => {
                for event in events {
                    let event_version = event.event_version.clone();
                    let event_sequence = event.sequence as i64;
                    let event_payload = event.payload.clone().to_string();
                    let event_metadata = event.metadata.to_string().clone();

                    sqlx::query!(
                        r#"
                        INSERT INTO events (
                            aggregate_type,
                            aggregate_id,
                            sequence,
                            event_type,
                            event_version,
                            payload,
                            metadata
                        )
                        VALUES ($1, $2, $3, $4, $5, $6, $7)
                        "#,
                        event.aggregate_type,
                        event.aggregate_id,
                        event_sequence,
                        event.event_type,
                        event_version,
                        event_payload,
                        event_metadata,
                    )
                    .execute(&mut *tx)
                    .await
                    .map_err(SqliteAggregateError::from)?;
                }
                Ok(())
            }
            Some(_) => {
                todo!()
            }
        };
        tx.commit().await.map_err(SqliteAggregateError::from)?;

        res
    }

    async fn stream_events<A: Aggregate>(
        &self,
        aggregate_id: &str,
    ) -> Result<ReplayStream, PersistenceError> {
        let owned_pool = self.pool.clone();
        let owned_aggregate_id = aggregate_id.to_string().clone();
        let (mut feed, stream) = ReplayStream::new(STREAM_CHANNEL_SIZE);
        let aggregate_type = A::aggregate_type();
        tokio::spawn(async move {
            let query = sqlx::query!(
                r#"
                SELECT
                    aggregate_type,
                    aggregate_id,
                    sequence,
                    event_type,
                    event_version,
                    payload,
                    metadata
                FROM events
                WHERE
                    aggregate_type = $1 AND
                    aggregate_id = $2
                ORDER BY sequence"#,
                aggregate_type,
                owned_aggregate_id
            );
            let mut rows = query.fetch(&owned_pool);

            while let Some(row) = rows.try_next().await.unwrap() {
                let event = Ok(SerializedEvent {
                    aggregate_id: owned_aggregate_id.clone(),
                    aggregate_type: row.aggregate_type,
                    event_version: row.event_version,
                    payload: serde_json::Value::String(row.payload),
                    metadata: serde_json::Value::String(row.metadata),
                    sequence: row.sequence as usize,
                    event_type: row.event_type,
                });
                feed.push(event).await.expect("Failed to push event to stream");
            }
        });
        Ok(stream)
    }

    async fn stream_all_events<A: Aggregate>(&self) -> Result<ReplayStream, PersistenceError> {
        let owned_pool = self.pool.clone();
        let (mut feed, stream) = ReplayStream::new(STREAM_CHANNEL_SIZE);
        let aggregate_type = A::aggregate_type();
        tokio::spawn(async move {
            let query = sqlx::query!(
                r#"
                SELECT
                    aggregate_type,
                    aggregate_id,
                    sequence,
                    event_type,
                    event_version,
                    payload,
                    metadata
                FROM events
                WHERE
                    aggregate_type = $1
                ORDER BY sequence"#,
                aggregate_type,
            );
            let mut rows = query.fetch(&owned_pool);

            while let Some(row) = rows.try_next().await.unwrap() {
                let event = Ok(SerializedEvent {
                    aggregate_id: row.aggregate_id,
                    aggregate_type: row.aggregate_type,
                    event_version: row.event_version,
                    payload: serde_json::Value::String(row.payload),
                    metadata: serde_json::Value::String(row.metadata),
                    sequence: row.sequence as usize,
                    event_type: row.event_type,
                });
                feed.push(event).await.expect("Failed to push event to stream");
            }
        });
        Ok(stream)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    use cqrs_es::DomainEvent;

    use serde::{Deserialize, Serialize};
    use sqlx::{Pool, Sqlite, SqlitePool};
    use std::fmt::{Display, Formatter};
    use uuid::Uuid;

    const TEST_CONNECTION_STRING: &str = "sqlite::memory:";

    #[tokio::test]
    async fn test_persist() {
        let id = Uuid::new_v4().to_string();
        let event_repo: SqliteEventRepository =
            SqliteEventRepository::new(default_sqlite_pool(TEST_CONNECTION_STRING).await);

        let events = event_repo.get_events::<TestAggregate>(&id).await.unwrap();
        assert!(events.is_empty());

        event_repo
            .persist::<TestAggregate>(
                &[
                    test_event_envelope(&id, 1, TestEvent::Created(Created { id: id.clone() })),
                    test_event_envelope(
                        &id,
                        2,
                        TestEvent::Tested(Tested {
                            test_name: "a test was run".to_string(),
                        }),
                    ),
                ],
                None,
            )
            .await
            .unwrap();
        let events = event_repo.get_events::<TestAggregate>(&id).await.unwrap();
        assert_eq!(2, events.len());
        events.iter().for_each(|e| assert_eq!(&id, &e.aggregate_id));
    }

    #[tokio::test]
    async fn test_persist_lock_error() {
        let id = Uuid::new_v4().to_string();
        let event_repo: SqliteEventRepository =
            SqliteEventRepository::new(default_sqlite_pool(TEST_CONNECTION_STRING).await);
        let result = event_repo
            .persist::<TestAggregate>(
                &[
                    test_event_envelope(
                        &id,
                        2,
                        TestEvent::ChangeDescription("this should not persist".to_string()),
                    ),
                    test_event_envelope(
                        &id,
                        2,
                        TestEvent::ChangeDescription("bad sequence number".to_string()),
                    ),
                ],
                None,
            )
            .await
            .unwrap_err();

        match result {
            PersistenceError::OptimisticLockError => (),
            _ => panic!("invalid error result found during insert: {}", result),
        };

        let events = event_repo.get_events::<TestAggregate>(&id).await.unwrap();
        assert_eq!(0, events.len());

        verify_replay_stream(&id, event_repo).await;
    }

    #[tokio::test]
    async fn test_stream_events() {
        let id = Uuid::new_v4().to_string();
        let other_id = Uuid::new_v4().to_string();

        let event_repo: SqliteEventRepository =
            SqliteEventRepository::new(default_sqlite_pool(TEST_CONNECTION_STRING).await);
        event_repo
            .persist::<TestAggregate>(
                &[
                    test_event_envelope(
                        &id,
                        1,
                        TestEvent::ChangeDescription("this should persist".to_string()),
                    ),
                    test_event_envelope(
                        &id,
                        2,
                        TestEvent::ChangeDescription("this should also persist".to_string()),
                    ),
                    test_event_envelope(
                        &other_id,
                        1,
                        TestEvent::ChangeDescription("this should also persist".to_string()),
                    ),
                ],
                None,
            )
            .await.expect("Failed to persist events");
        
        let mut stream = event_repo
            .stream_events::<TestAggregate>(&id)
            .await
            .unwrap();

        let mut found_in_stream = 0;
        while let Some(_) = stream.next::<TestAggregate>().await {
            found_in_stream += 1;
        }
        assert_eq!(found_in_stream, 2);
    }

    #[tokio::test]
    async fn test_stream_all_events() {
        let id = Uuid::new_v4().to_string();
        let other_id = Uuid::new_v4().to_string();
        let event_repo: SqliteEventRepository =
            SqliteEventRepository::new(default_sqlite_pool(TEST_CONNECTION_STRING).await);
        event_repo
            .persist::<TestAggregate>(
                &[
                    test_event_envelope(
                        &id,
                        1,
                        TestEvent::ChangeDescription("this should persist".to_string()),
                    ),
                    test_event_envelope(
                        &id,
                        2,
                        TestEvent::ChangeDescription("this should also persist".to_string()),
                    ),
                    test_event_envelope(
                        &other_id,
                        1,
                        TestEvent::ChangeDescription("this should also persist".to_string()),
                    ),
                ],
                None,
            )
            .await.expect("Failed to persist events");
        let mut stream = event_repo
            .stream_all_events::<TestAggregate>()
            .await
            .unwrap();
        let mut found_in_stream = 0;
        while let Some(_) = stream.next::<TestAggregate>().await {
            found_in_stream += 1;
        }
        assert_eq!(found_in_stream, 3);
    }

    #[tokio::test]
    async fn snapshot_repositories_default_none() {
        let id = uuid::Uuid::new_v4().to_string();
        let event_repo: SqliteEventRepository =
            SqliteEventRepository::new(default_sqlite_pool(TEST_CONNECTION_STRING).await);
        let snapshot = event_repo.get_snapshot::<TestAggregate>(&id).await.unwrap();
        assert_eq!(None, snapshot);
    }

    #[tokio::test]
    async fn snapshot_repositories_aggregate_state() {
        let id = uuid::Uuid::new_v4().to_string();
        let event_repo: SqliteEventRepository =
            SqliteEventRepository::new(default_sqlite_pool(TEST_CONNECTION_STRING).await);

        let test_description = "some test snapshot here".to_string();

        event_repo
            .persist::<TestAggregate>(
                &[
                    test_event_envelope(&id, 1, TestEvent::Created(Created { id: id.clone() })),
                    test_event_envelope(
                        &id,
                        2,
                        TestEvent::ChangeDescription(test_description.clone()),
                    ),
                ],
                None,
            )
            .await
            .unwrap();

        let snapshot = event_repo.get_snapshot::<TestAggregate>(&id).await.unwrap();
        assert_eq!(
            Some(snapshot_context(
                id.clone(),
                0,
                1,
                serde_json::to_value(TestAggregate {
                    id: id.clone(),
                    description: test_description.clone(),
                })
                .unwrap()
            )),
            snapshot
        );
    }

    #[tokio::test]
    async fn snapshot_repositories_sequence_iterated() {
        let id = uuid::Uuid::new_v4().to_string();
        let event_repo: SqliteEventRepository =
            SqliteEventRepository::new(default_sqlite_pool(TEST_CONNECTION_STRING).await);

        let test_aggregate = serde_json::to_value(TestAggregate {
            id: id.clone(),
            description: "an old test description".to_string(),
        })
        .unwrap();

        event_repo
            .persist::<TestAggregate>(
                &[
                    test_event_envelope(&id, 1, TestEvent::Created(Created { id: id.clone() })),
                    test_event_envelope(
                        &id,
                        2,
                        TestEvent::ChangeDescription(
                            "a test description that should be saved".to_string(),
                        ),
                    ),
                ],
                Some((id.clone(), test_aggregate, 3)),
            )
            .await
            .unwrap();

        let snapshot = event_repo.get_snapshot::<TestAggregate>(&id).await.unwrap();
        assert_eq!(
            Some(snapshot_context(
                id.clone(),
                0,
                2,
                serde_json::to_value(TestAggregate {
                    id: id.clone(),
                    description: "a test description that should be saved".to_string(),
                })
                .unwrap()
            )),
            snapshot
        );
    }

    #[tokio::test]
    async fn snapshot_repositories_sequence_not_iterated() {
        let id = uuid::Uuid::new_v4().to_string();
        let event_repo: SqliteEventRepository =
            SqliteEventRepository::new(default_sqlite_pool(TEST_CONNECTION_STRING).await);

        // sequence out of order or not iterated, does not update
        let result = event_repo
            .persist::<TestAggregate>(
                &[
                    test_event_envelope(
                        &id,
                        2,
                        TestEvent::ChangeDescription(
                            "a test description that should not be saved".to_string(),
                        ),
                    ),
                    test_event_envelope(&id, 1, TestEvent::Created(Created { id: id.clone() })),
                ],
                None,
            )
            .await
            .unwrap_err();

        match result {
            PersistenceError::OptimisticLockError => (),
            _ => panic!("invalid error result found during insert: {}", result),
        };

        let snapshot = event_repo.get_snapshot::<TestAggregate>(&id).await.unwrap();
        assert_eq!(
            Some(snapshot_context(
                id.clone(),
                0,
                2,
                serde_json::to_value(TestAggregate {
                    id: id.clone(),
                    description: "a test description that should be saved".to_string(),
                })
                .unwrap()
            )),
            snapshot
        );
    }

    async fn default_sqlite_pool(test_connection_string: &str) -> Pool<Sqlite> {
        let pool = SqlitePool::connect(test_connection_string)
            .await
            .expect("failed to connect to sqlite");

        sqlx::migrate!("./migrations/")
            .run(&pool)
            .await
            .expect("failed to migrate sqlite");

        pool
    }

    async fn verify_replay_stream(id: &str, event_repo: SqliteEventRepository) {
    }

    pub(crate) fn test_event_envelope(
        id: &str,
        sequence: usize,
        event: TestEvent,
    ) -> SerializedEvent {
        let payload: Value = serde_json::to_value(&event).unwrap();
        SerializedEvent {
            aggregate_id: id.to_string(),
            sequence,
            aggregate_type: TestAggregate::aggregate_type().to_string(),
            event_type: event.event_type().to_string(),
            event_version: event.event_version().to_string(),
            payload,
            metadata: Default::default(),
        }
    }

    pub(crate) fn snapshot_context(
        aggregate_id: String,
        current_sequence: usize,
        current_snapshot: usize,
        aggregate: Value,
    ) -> SerializedSnapshot {
        SerializedSnapshot {
            aggregate_id,
            aggregate,
            current_sequence,
            current_snapshot,
        }
    }

    #[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
    pub(crate) enum TestEvent {
        Created(Created),
        Tested(Tested),
        ChangeDescription(String),
    }

    #[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
    pub(crate) struct Created {
        pub id: String,
    }

    #[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
    pub(crate) struct Tested {
        pub test_name: String,
    }

    impl DomainEvent for TestEvent {
        fn event_type(&self) -> String {
            match self {
                TestEvent::Created(_) => "Created".to_string(),
                TestEvent::Tested(_) => "Tested".to_string(),
                TestEvent::ChangeDescription(_) => "ChangeDescription".to_string(),
            }
        }

        fn event_version(&self) -> String {
            "1.0".to_string()
        }
    }

    #[derive(Debug, PartialEq)]
    pub(crate) struct TestError(String);

    #[derive(Debug)]
    pub(crate) struct TestServices;

    impl Display for TestError {
        fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
            write!(f, "{}", self.0)
        }
    }

    impl std::error::Error for TestError {}

    pub(crate) enum TestCommand {}

    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    pub(crate) struct TestAggregate {
        pub(crate) id: String,
        pub(crate) description: String,
    }

    #[async_trait]
    impl Aggregate for TestAggregate {
        type Command = TestCommand;
        type Event = TestEvent;
        type Error = TestError;
        type Services = TestServices;

        fn aggregate_type() -> String {
            "TestAggregate".to_string()
        }

        async fn handle(
            &self,
            _command: Self::Command,
            _services: &Self::Services,
        ) -> Result<Vec<Self::Event>, Self::Error> {
            Ok(vec![])
        }

        fn apply(&mut self, event: Self::Event) {
            self.description = match event {
                TestEvent::Created(_) => "i was just created".to_string(),
                TestEvent::Tested(_) => "i was just tested".to_string(),
                TestEvent::ChangeDescription(description) => description,
            };
        }
    }

    impl Default for TestAggregate {
        fn default() -> Self {
            TestAggregate {
                id: "".to_string(),
                description: "".to_string(),
            }
        }
    }
}
