use clap::Parser;

use crate::app::App;
use crate::clap_arguments::Args;
use crate::utils::{init_terminal, restore_terminal};

mod app;
mod utils;
mod clap_arguments;

fn main() -> anyhow::Result<()> {
    // TODO: Cleanup main function

    let args = Args::parse();

    let terminal = init_terminal()?;

    let mut app = match args.input_name {
        None => App::make_temporary(),
        Some(input_name) => match App::make_saved(&input_name) {
            Ok(app) => {
                app
            }
            Err(error) => {
                restore_terminal()?;
                return Err(error);
            }
        },
    };
    let final_message = app.run(terminal)?;

    restore_terminal()?;

    if !final_message.is_empty() {
        println!("{}", final_message);
    }

    Ok(())
}
