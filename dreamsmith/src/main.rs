use anyhow::{Result};
use binrw::*;
use dreamsmith::*;

/*
fn write_a18(name: String, page: &AudioClip) -> Result<()> {
    let data:Vec<u8> = vec![];
    let mut cursor = io::Cursor::new(data);
    page.write(&mut cursor)?;
    Ok(std::fs::write(name, cursor.into_inner())?)//.context("Failed to write a18")
}
*/

fn main() -> Result<()>{
    //let image = std::fs::read("test_data/BSLSBSBaseF.bin")?;
    let image = std::fs::read("test_data/Paw_Patrol-All-Star_Pups.bin")?;
    let mut cursor = io::Cursor::new(image);
    let book = StoryBook::read(&mut cursor)?;
    println!("{book:#x?}");
    println!("{book}");
    Ok(())
}
