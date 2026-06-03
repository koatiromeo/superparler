pub mod db;
pub mod models;

#[cfg(test)]
mod tests {
    use sqlx::SqlitePool;
    use crate::storage::models;

    /// Open an in-memory SQLite pool, run migrations, then test insert/select/delete.
    async fn test_pool() -> SqlitePool {
        let pool = SqlitePool::connect("sqlite::memory:").await.expect("in-memory pool");
        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .expect("migrations");
        pool
    }

    #[tokio::test]
    async fn test_insert_and_list() {
        let pool = test_pool().await;

        let t = models::insert_transcription(&pool, "Bonjour le monde", 4000, "local", "fr", false)
            .await
            .expect("insert");

        assert_eq!(t.text, "Bonjour le monde");
        assert_eq!(t.engine, "local");
        assert_eq!(t.enhanced, 0);

        let rows = models::list_transcriptions(&pool, 10, 0).await.expect("list");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, t.id);
    }

    #[tokio::test]
    async fn test_delete() {
        let pool = test_pool().await;
        let t = models::insert_transcription(&pool, "À supprimer", 1000, "groq", "fr", true)
            .await
            .expect("insert");

        models::delete_transcription(&pool, &t.id).await.expect("delete");
        let rows = models::list_transcriptions(&pool, 10, 0).await.expect("list after delete");
        assert!(rows.is_empty());
    }

    #[tokio::test]
    async fn test_clear_all() {
        let pool = test_pool().await;
        models::insert_transcription(&pool, "A", 100, "local", "fr", false).await.unwrap();
        models::insert_transcription(&pool, "B", 200, "groq", "en", true).await.unwrap();

        models::clear_all(&pool).await.expect("clear");
        let rows = models::list_transcriptions(&pool, 10, 0).await.unwrap();
        assert!(rows.is_empty());
    }

    #[tokio::test]
    async fn test_list_pagination() {
        let pool = test_pool().await;
        for i in 0..5 {
            models::insert_transcription(&pool, &format!("item {i}"), 500, "local", "fr", false)
                .await
                .unwrap();
        }
        let page1 = models::list_transcriptions(&pool, 3, 0).await.unwrap();
        let page2 = models::list_transcriptions(&pool, 3, 3).await.unwrap();
        assert_eq!(page1.len(), 3);
        assert_eq!(page2.len(), 2);
    }
}
