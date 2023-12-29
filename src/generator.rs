use ic_cketh_minter::{assets::AssetWithPath, eth_logs::MintEvent, state::event};
use image::{codecs::png::PngEncoder, ColorType, ImageBuffer, ImageEncoder, RgbImage};
use serde_json::{json, to_vec};

pub fn generator(randomness: [u8; 32], event: MintEvent) -> Vec<AssetWithPath> {
    // create vector to hold assets
    let mut assets: Vec<AssetWithPath> = Vec::new();

    let metadata = generate_metadata(randomness, &event);
    assets.push(metadata);

    let image = generate_png(randomness, &event);
    assets.push(image);

    assets
}

fn generate_metadata(randomness: [u8; 32], event: &MintEvent) -> AssetWithPath {
    // create JSON metadata with serde_json
    let json_literal = json!({
        "name": "John Doe",
        "age": 30,
        "is_admin": false,
        "phones": ["+44 1234567", "+44 2345678"]
    });

    // Serialize the JSON value to a Vec<u8>
    let byte_vec: Vec<u8> = match to_vec(&json_literal) {
        Ok(vec) => vec,
        Err(_) => {
            ic_cdk::trap("Failed to serialize JSON");
        }
    };
    // return Asset
    AssetWithPath {
        path: format!("/metadata/{}.json", event.token_id),
        bytes: byte_vec,
        headers: vec![("Content-Type".to_string(), "application/json".to_string())],
    }
}

fn generate_png(randomness: [u8; 32], event: &MintEvent) -> AssetWithPath {
    // Create a black image
    let mut img: RgbImage = ImageBuffer::new(100, 100);
    img.fill(1);

    // Serialize the image to PNG format
    let mut bytes: Vec<u8> = Vec::new();
    PngEncoder::new(&mut bytes)
        .write_image(&img, img.width(), img.height(), ColorType::Rgb8)
        .expect("Failed to encode the image as PNG");

    AssetWithPath {
        path: format!("/media/{}.png", event.token_id),
        bytes,
        headers: vec![("Content-Type".into(), "image/png".into())],
    }
}
