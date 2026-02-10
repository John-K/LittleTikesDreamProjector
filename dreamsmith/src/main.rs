use anyhow::{Result};
use binrw::*;

mod storybook;
use storybook::*;

/*
fn write_a18(name: String, page: &AudioClip) -> Result<()> {
    let data:Vec<u8> = vec![];
    let mut cursor = io::Cursor::new(data);
    page.write(&mut cursor)?;
    Ok(std::fs::write(name, cursor.into_inner())?)//.context("Failed to write a18")
}
*/

fn main() -> Result<()>{
    let args: Vec<String> = std::env::args().collect();
    let path = args
        .get(1)
        .expect("Usage: dreamsmith <storybook.bin>");
    let image = std::fs::read(path)?;
    let mut cursor = io::Cursor::new(image);
    let book = StoryBook::read(&mut cursor)?;
    println!("{book:#x?}");
    println!("{book}");
    Ok(())
}
