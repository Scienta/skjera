#[macro_export] macro_rules! id_type {
    ($type_name:ident) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, sqlx::Type, serde::Serialize, serde::Deserialize)]
        #[sqlx(transparent)]
        pub struct $type_name(pub i64);

        impl From<i64> for $type_name {
            fn from(id: i64) -> Self {
                $type_name(id)
            }
        }

        impl From<$type_name> for i64 {
            fn from(id: $type_name) -> Self {
                id.0
            }
        }

        impl std::fmt::Display for $type_name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.0)
            }
        }
    };
}
