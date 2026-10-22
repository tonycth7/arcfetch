// Minimal JSONC parser.

#[derive(Debug, Clone)]
pub enum Value {
    Null,
    Bool(bool),
    Num(f64),
    Str(String),
    Arr(Vec<Value>),
    Obj(Vec<(String, Value)>),
}

impl Value {
    pub fn get(&self, key: &str) -> Option<&Value> {
        match self {
            Value::Obj(fields) => fields.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }
    pub fn as_str(&self) -> Option<&str> {
        match self { Value::Str(s) => Some(s), _ => None }
    }
    pub fn as_usize(&self) -> Option<usize> {
        match self {
            Value::Num(n) if *n >= 0.0 && n.fract() == 0.0 => Some(*n as usize),
            _ => None,
        }
    }
    pub fn as_array(&self) -> Option<&[Value]> {
        match self { Value::Arr(v) => Some(v), _ => None }
    }
}

pub fn parse(src: &str) -> Result<Value, String> {
    let mut p = Parser { b: src.as_bytes(), i: 0 };
    p.skip_ws();
    let v = p.value(0)?;
    p.skip_ws();
    if p.i != p.b.len() { return Err(p.err("trailing input")); }
    Ok(v)
}

struct Parser<'a> { b: &'a [u8], i: usize }

impl<'a> Parser<'a> {
    fn err(&self, msg: &str) -> String {
        let line = self.b[..self.i.min(self.b.len())].iter().filter(|&&c| c == b'\n').count() + 1;
        format!("{msg} at line {line}")
    }
    fn peek(&self) -> Option<u8> { self.b.get(self.i).copied() }
    fn bump(&mut self) -> Option<u8> { let c = self.peek()?; self.i += 1; Some(c) }
    fn skip_ws(&mut self) {
        loop {
            while self.peek().is_some_and(|c| c.is_ascii_whitespace()) { self.i += 1; }
            if self.peek() == Some(b'/') {
                match self.b.get(self.i + 1) {
                    Some(b'/') => { self.i += 2; while self.peek().is_some_and(|c| c != b'\n') { self.i += 1; } continue; }
                    Some(b'*') => { self.i += 2; while self.i + 1 < self.b.len() { if self.b[self.i]==b'*' && self.b[self.i+1]==b'/' { self.i += 2; break; } self.i += 1; } continue; }
                    _ => {}
                }
            }
            break;
        }
    }
    fn value(&mut self, depth: usize) -> Result<Value, String> {
        if depth > 128 { return Err(self.err("nesting too deep")); }
        self.skip_ws();
        match self.peek() {
            Some(b'{') => self.object(depth + 1),
            Some(b'[') => self.array(depth + 1),
            Some(b'"') => Ok(Value::Str(self.string()?)),
            Some(b't') => { self.literal(b"true")?; Ok(Value::Bool(true)) }
            Some(b'f') => { self.literal(b"false")?; Ok(Value::Bool(false)) }
            Some(b'n') => { self.literal(b"null")?; Ok(Value::Null) }
            Some(c) if c == b'-' || c.is_ascii_digit() => self.number(),
            _ => Err(self.err("expected a value")),
        }
    }
    fn literal(&mut self, lit: &[u8]) -> Result<(), String> {
        if self.b.get(self.i..self.i + lit.len()) == Some(lit) { self.i += lit.len(); Ok(()) }
        else { Err(self.err("invalid literal")) }
    }
    fn number(&mut self) -> Result<Value, String> {
        let start = self.i;
        while self.peek().is_some_and(|c| c == b'-' || c == b'+' || c == b'.' || c == b'e' || c == b'E' || c.is_ascii_digit()) { self.i += 1; }
        let s = std::str::from_utf8(&self.b[start..self.i]).map_err(|_| self.err("invalid number"))?;
        s.parse::<f64>().map(Value::Num).map_err(|_| self.err("invalid number"))
    }
    fn string(&mut self) -> Result<String, String> {
        self.bump();
        let mut out = String::new();
        loop {
            match self.bump() {
                Some(b'"') => return Ok(out),
                Some(b'\\') => match self.bump() {
                    Some(b'"') => out.push('"'),
                    Some(b'\\') => out.push('\\'),
                    Some(b'/') => out.push('/'),
                    Some(b'b') => out.push('\u{0008}'),
                    Some(b'f') => out.push('\u{000c}'),
                    Some(b'n') => out.push('\n'),
                    Some(b'r') => out.push('\r'),
                    Some(b't') => out.push('\t'),
                    Some(b'u') => {
                        let hi = self.hex4()?;
                        let cp = if (0xD800..=0xDBFF).contains(&hi) && self.peek() == Some(b'\\') && self.b.get(self.i + 1) == Some(&b'u') {
                            self.i += 2; let lo = self.hex4()?; 0x10000 + ((hi - 0xD800) << 10) + (lo - 0xDC00)
                        } else { hi };
                        out.push(char::from_u32(cp).unwrap_or('\u{fffd}'));
                    }
                    _ => return Err(self.err("invalid escape")),
                },
                Some(c) if c < 0x20 => return Err(self.err("control character in string")),
                Some(c) => out.push(c as char),
                None => return Err(self.err("unterminated string")),
            }
        }
    }
    fn hex4(&mut self) -> Result<u32, String> {
        let mut n = 0u32;
        for _ in 0..4 {
            let c = self.bump().ok_or_else(|| self.err("short unicode escape"))?;
            let d = (c as char).to_digit(16).ok_or_else(|| self.err("invalid unicode escape"))?;
            n = n * 16 + d;
        }
        Ok(n)
    }
    fn array(&mut self, depth: usize) -> Result<Value, String> {
        self.bump();
        let mut out = Vec::new();
        loop {
            self.skip_ws();
            if self.peek() == Some(b']') { self.bump(); return Ok(Value::Arr(out)); }
            out.push(self.value(depth)?);
            self.skip_ws();
            match self.bump() {
                Some(b',') => continue,
                Some(b']') => return Ok(Value::Arr(out)),
                _ => return Err(self.err("expected ',' or ']'")),
            }
        }
    }
    fn object(&mut self, depth: usize) -> Result<Value, String> {
        self.bump();
        let mut out = Vec::new();
        loop {
            self.skip_ws();
            if self.peek() == Some(b'}') { self.bump(); return Ok(Value::Obj(out)); }
            if self.peek() != Some(b'"') { return Err(self.err("expected object key")); }
            let key = self.string()?;
            self.skip_ws();
            if self.bump() != Some(b':') { return Err(self.err("expected ':'")); }
            let val = self.value(depth)?;
            out.push((key, val));
            self.skip_ws();
            match self.bump() {
                Some(b',') => continue,
                Some(b'}') => return Ok(Value::Obj(out)),
                _ => return Err(self.err("expected ',' or '}'")),
            }
        }
    }
}
PY
echo done