use chrono::prelude::Local;

use std::io::Write;
use std::sync::mpsc;

struct ExitOnDrop;

impl Drop for ExitOnDrop {

  fn drop(&mut self) {
    ::std::process::exit(1);
  }

}

fn run_logging_output_thread(receiver: mpsc::Receiver<String>) {

  let _exit_on_drop = ExitOnDrop;

  let mut stdout = ::std::io::stdout();

  loop {
    match receiver.recv() {
      Ok(msg) => {
        stdout.write(msg.as_bytes())
          .expect("run_logging_output_thread error writing msg to stdout");
        loop {
          match receiver.try_recv() {
            Ok(next_msg) => {
              stdout.write(next_msg.as_bytes())
                .expect("run_logging_output_thread error writing next_msg to stdout");
            },
            Err(mpsc::TryRecvError::Empty) => break,
            Err(mpsc::TryRecvError::Disconnected) => {
              stdout.write("run_logging_output_thread try_recv disconnected error".as_bytes())
                .expect("run_logging_output_thread error writing to stdout");
            }
          }
        }
      },
      Err(e) => {
        stdout.write(format!("run_logging_output_thread recv error {}\n", e).as_bytes())
          .expect("run_logging_output_thread error writing to stdout");
      }
    }

    stdout.flush().expect("run_logging_output_thread error flushing stdout");
  }
}

pub fn initialize_logging() -> Result<(), Box<::std::error::Error>> {

  let (sender, receiver) = mpsc::channel();

  ::std::thread::Builder::new().name("logging_output".to_string()).spawn(move || {
    run_logging_output_thread(receiver);
  })?;

  ::fern::Dispatch::new()
    .level(::log::LevelFilter::Info)
    .format(|out, message, record| {
      out.finish(
        format_args!("{} [{}] {} {} - {}",
          Local::now().format("%Y-%m-%d %H:%M:%S%.3f %z"),
          ::std::thread::current().name().unwrap_or("UNKNOWN"),
          record.level(),
          record.target(),
          message
        )
      )
    })
    .chain(sender)
    .apply()?;

  Ok(())
}
