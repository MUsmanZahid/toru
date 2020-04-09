mod lib;

use lib::{Instance, ToruError};

fn main() -> Result<(), ToruError> {
    let instance = Instance::from_args()?;
    instance.run();

    Ok(())
}
