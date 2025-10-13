use std::time::UNIX_EPOCH;

pub fn process_complete_chunk(chunk: &[u8], add_time_prefix: bool) -> Vec<Vec<u8>> {
    return if add_time_prefix {
        let current_time = std::time::SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("unix epoch to be earlier")
            .as_secs();
        let time_prefix = format!("{current_time},");

        inner(chunk, time_prefix.as_bytes())
    } else {
        inner(chunk, &[])
    };

    fn inner(chunk: &[u8], prefix: &[u8]) -> Vec<Vec<u8>> {
        chunk
            .split(|c| *c == b'\n')
            .filter_map(|mut line| {
                if let Some(b'\r') = line.last() {
                    line = &line[..(line.len() - 1)];
                }
                if line.is_empty() {
                    return None;
                }
                Some([prefix, line, b"\n"].concat())
            })
            .collect()
    }
}
