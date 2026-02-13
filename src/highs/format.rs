use clap::ValueEnum;

#[derive(Clone, Debug, ValueEnum)]
pub enum Format {
    Txt,
    Csv,
}

impl std::fmt::Display for Format {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Format::Txt => write!(f, "txt"),
            Format::Csv => write!(f, "csv"),
        }
    }
}
