use chrono;
use log::{ LevelFilter, SetLoggerError };
use std::fs::{ File, OpenOptions };
use std::io::Write;

pub struct FileLogger {
    file: File,
}

impl log::Log for FileLogger {
    fn enabled(&self, _metadata: &log::Metadata) -> bool {
        true
    }
    fn log(&self, record: &log::Record) {
        if self.enabled(record.metadata()) {
            let mut file = self.file.try_clone().expect("Failed to clone file handle");
            let log_message = format!(
                "{} - {}: {}",
                chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
                record.level(),
                record.args()
            );
            writeln!(file, "{}", log_message).expect("Failed to write to log file");
        }
    }

    fn flush(&self) {
        self.file.sync_all().expect("Failed to flush log file");
    }
}

pub fn init_logger(app_data_dir: &std::path::Path) -> Result<(), SetLoggerError> {
    let logs_dir = app_data_dir.join("logs");
    std::fs::create_dir_all(&logs_dir).expect("Failed to create logs directory");

    let log_path = logs_dir.join("app.log");
    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)
        .expect("Failed to open log file");

    let logger = Box::new(FileLogger { file });
    unsafe {
        log::set_logger_racy(Box::leak(logger))?;
    }
    log::set_max_level(LevelFilter::Debug);
    Ok(())
}

pub fn info(message: &str) {
    let log_message = format!(
        "{} - INFO: {}",
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
        message
    );
    println!("{}", log_message);

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open("app.log")
        .expect("Failed to open log file");
    writeln!(file, "{}", log_message).expect("Failed to write to log file");
}

#[macro_export]
macro_rules! info {
    ($($arg:tt)*) => {
        crate::utils::logger::info(&format!($($arg)*))
    };
}
