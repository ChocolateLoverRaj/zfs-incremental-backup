// use std::path::Path;

// use serde::{Deserialize, Serialize};
// use tokio::io::AsyncReadExt;

// pub struct SerdeJsonFile<T: Serialize + Deserialize<'a>> {
//     string: String,
//     pub data: T,
// }

// impl<'a, T: Serialize + Deserialize<'a> + Clone> SerdeJsonFile<'a, T> {
//     pub async fn open(path: impl AsRef<Path>) -> anyhow::Result<Self> {
//         let mut file = tokio::fs::File::open(path).await?;
//         let mut string = String::default();
//         file.read_to_string(&mut string).await?;
//         let data = serde_json::from_str::<T>(&string)?;
//         Ok(Self { data, string: &str })
//     }
// }
