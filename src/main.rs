use anyhow::Result;
use tokio::runtime::Builder;

use crate::app::App;

mod app;
mod download;

fn main() -> Result<()> {
    let rt = Builder::new_current_thread().enable_all().build()?;
    rt.block_on(App::run())?;

    Ok(())
}
