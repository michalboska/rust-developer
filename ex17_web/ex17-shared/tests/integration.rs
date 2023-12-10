use anyhow::Error;
use lazy_static::lazy_static;
use rand::random;
use tokio::fs::{remove_file, File};
use tokio::io::AsyncWriteExt;

use ex17_shared::message::Message;

lazy_static! {
    static ref CONTENT: Vec<u8> = vec![1, 2, 3, 4, 5];
}

async fn delete_test_file(file_name: &str) {
    let _ = remove_file(file_name).await;
}

async fn create_test_file(file_name: &str) -> Result<String, Error> {
    let rand = random::<u32>();
    let path = format!(
        "{}/{}{}",
        std::env::temp_dir().as_os_str().to_str().unwrap(),
        rand,
        file_name
    );
    delete_test_file(&path).await;
    let mut file = File::create(&path).await?;
    let _ = file.write(&CONTENT).await?;
    Ok(path)
}

#[tokio::test]
async fn test_image() -> Result<(), Error> {
    let name = "image.png";
    let full_path = create_test_file(name).await?;
    let message = Message::from_str(&format!(".image {}", full_path))
        .await
        .unwrap();
    match message {
        Message::Image(vec) => {
            assert_eq!(CONTENT.as_ref(), vec);
        }
        _ => {
            panic!("Wrong type: {:?}", message);
        }
    }
    Ok(())
}

#[tokio::test]
async fn test_file() -> Result<(), Error> {
    let name = "file.bin";
    let full_path = create_test_file(name).await?;
    let message = Message::from_str(&format!(".file {}", full_path))
        .await
        .unwrap();
    match message {
        Message::File(file_name, vec) => {
            assert_eq!(full_path, file_name);
            assert_eq!(CONTENT.as_ref(), vec);
        }
        _ => {
            panic!("Wrong type: {:?}", message);
        }
    }
    Ok(())
}
