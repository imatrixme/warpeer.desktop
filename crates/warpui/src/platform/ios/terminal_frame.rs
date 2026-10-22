#[derive(Clone, Debug, Default, PartialEq)]
pub struct TerminalFrame {
    pub rows: u16,
    pub columns: u16,
    pub cells: Vec<TerminalCell>,
    pub cursor: Option<(u16, u16)>,
    pub alternate_screen: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TerminalCell {
    pub row: u16,
    pub column: u16,
    pub text: String,
    pub foreground: [u8; 4],
    pub background: [u8; 4],
    pub wide: bool,
    pub underline: bool,
}
