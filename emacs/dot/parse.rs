use super::ir::{Cluster, Decision, Dir, Edge, Morphism, Node, NodeKind, Pattern, Stmt};

#[derive(Debug)]
pub struct ParseError {
    pub pos: usize,
    pub msg: String,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "parse error at byte {}: {}", self.pos, self.msg)
    }
}

impl std::error::Error for ParseError {}

struct Parser<'a> {
    input: &'a [u8],
    pos: usize,
}

impl<'a> Parser<'a> {
    fn new(input: &'a str) -> Self {
        Self {
            input: input.as_bytes(),
            pos: 0,
        }
    }

    fn err(&self, msg: impl Into<String>) -> ParseError {
        ParseError {
            pos: self.pos,
            msg: msg.into(),
        }
    }

    fn at_end(&self) -> bool {
        self.pos >= self.input.len()
    }

    fn peek(&self) -> Option<u8> {
        self.input.get(self.pos).copied()
    }

    fn peek2(&self) -> Option<u8> {
        self.input.get(self.pos + 1).copied()
    }

    fn advance(&mut self) -> Option<u8> {
        let ch = self.peek()?;
        self.pos += 1;
        Some(ch)
    }

    fn skip_ws(&mut self) {
        loop {
            match self.peek() {
                Some(b' ' | b'\t' | b'\n' | b'\r') => {
                    self.pos += 1;
                }
                Some(b'/') if self.peek2() == Some(b'/') => {
                    self.pos += 2;
                    while let Some(ch) = self.peek() {
                        self.pos += 1;
                        if ch == b'\n' {
                            break;
                        }
                    }
                }
                _ => break,
            }
        }
    }

    fn expect_byte(&mut self, expected: u8) -> Result<(), ParseError> {
        self.skip_ws();
        match self.advance() {
            Some(ch) if ch == expected => Ok(()),
            Some(ch) => Err(self.err(format!(
                "expected '{}', found '{}'",
                expected as char, ch as char
            ))),
            None => Err(self.err(format!("expected '{}', found EOF", expected as char))),
        }
    }

    fn try_byte(&mut self, expected: u8) -> bool {
        self.skip_ws();
        if self.peek() == Some(expected) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    fn try_word(&mut self, word: &str) -> bool {
        self.skip_ws();
        let start = self.pos;
        for &expected in word.as_bytes() {
            match self.advance() {
                Some(ch) if ch == expected => {}
                _ => {
                    self.pos = start;
                    return false;
                }
            }
        }
        if let Some(ch) = self.peek() {
            if ch.is_ascii_alphanumeric() || ch == b'_' {
                self.pos = start;
                return false;
            }
        }
        true
    }

    fn read_ident(&mut self) -> Result<String, ParseError> {
        self.skip_ws();
        let start = self.pos;
        while let Some(ch) = self.peek() {
            if ch.is_ascii_alphanumeric() || ch == b'_' {
                self.pos += 1;
            } else {
                break;
            }
        }
        if self.pos == start {
            return Err(self.err("expected identifier"));
        }
        Ok(String::from_utf8_lossy(&self.input[start..self.pos]).into_owned())
    }

    fn read_u32(&mut self) -> Result<u32, ParseError> {
        self.skip_ws();
        let start = self.pos;
        while let Some(ch) = self.peek() {
            if ch.is_ascii_digit() {
                self.pos += 1;
            } else {
                break;
            }
        }
        if self.pos == start {
            return Err(self.err("expected digits"));
        }
        let s = std::str::from_utf8(&self.input[start..self.pos]).unwrap();
        s.parse::<u32>()
            .map_err(|_| self.err(format!("invalid number: {s}")))
    }

    fn skip_balanced(&mut self) -> Result<(), ParseError> {
        let mut depth_paren = 0i32;
        let mut depth_bracket = 0i32;
        let mut depth_brace = 0i32;
        loop {
            match self.peek() {
                None => return Err(self.err("unexpected EOF in balanced expression")),
                Some(b'(') => {
                    depth_paren += 1;
                    self.pos += 1;
                }
                Some(b')') => {
                    if depth_paren <= 0 {
                        return Ok(());
                    }
                    depth_paren -= 1;
                    self.pos += 1;
                }
                Some(b'[') => {
                    depth_bracket += 1;
                    self.pos += 1;
                }
                Some(b']') => {
                    if depth_bracket <= 0 {
                        return Ok(());
                    }
                    depth_bracket -= 1;
                    self.pos += 1;
                }
                Some(b'{') => {
                    depth_brace += 1;
                    self.pos += 1;
                }
                Some(b'}') => {
                    if depth_brace <= 0 {
                        return Ok(());
                    }
                    depth_brace -= 1;
                    self.pos += 1;
                }
                Some(b'"') => {
                    self.pos += 1;
                    while let Some(ch) = self.advance() {
                        if ch == b'\\' {
                            self.advance();
                        } else if ch == b'"' {
                            break;
                        }
                    }
                }
                Some(b',') if depth_paren == 0 && depth_bracket == 0 && depth_brace == 0 => {
                    return Ok(());
                }
                _ => {
                    self.pos += 1;
                }
            }
        }
    }

    fn parse_morphism(&mut self) -> Result<Morphism, ParseError> {
        self.skip_ws();
        let _ = self.try_word("Morphism");
        let _ = self.try_byte(b':');
        let _ = self.try_byte(b':');
        let ident = self.read_ident()?;
        match ident.as_str() {
            "Iso" => Ok(Morphism::Iso),
            "SubIso" => Ok(Morphism::SubIso),
            "EpiMono" => Ok(Morphism::EpiMono),
            "Mono" => Ok(Morphism::Mono),
            "Epi" => Ok(Morphism::Epi),
            "Homo" => Ok(Morphism::Homo),
            _ => Err(self.err(format!("unknown morphism: {ident}"))),
        }
    }

    fn parse_node(&mut self, negated: bool) -> Result<Node, ParseError> {
        self.skip_ws();
        let start = self.pos;
        let ident = self.read_ident()?;

        match ident.as_str() {
            "N" => {
                self.expect_byte(b'(')?;
                self.skip_ws();
                if self.peek() == Some(b')') {
                    self.pos = start;
                    return Err(self.err("N() is not valid, use N_()"));
                }
                let id = self.read_u32()?;
                self.expect_byte(b')')?;
                let (has_val, has_pred) = self.parse_node_modifiers()?;
                Ok(Node {
                    kind: NodeKind::Free,
                    id: Some(id),
                    negated,
                    has_val,
                    has_pred,
                })
            }
            "N_" => {
                self.expect_byte(b'(')?;
                self.expect_byte(b')')?;
                let (has_val, has_pred) = self.parse_node_modifiers()?;
                Ok(Node {
                    kind: NodeKind::Free,
                    id: None,
                    negated,
                    has_val,
                    has_pred,
                })
            }
            "n" => {
                self.expect_byte(b'(')?;
                let id = self.read_u32()?;
                self.expect_byte(b')')?;
                let (has_val, has_pred) = self.parse_node_modifiers()?;
                Ok(Node {
                    kind: NodeKind::FreeRef,
                    id: Some(id),
                    negated,
                    has_val,
                    has_pred,
                })
            }
            "X" | "T" => {
                self.expect_byte(b'(')?;
                let id = self.read_u32()?;
                self.expect_byte(b')')?;
                let (has_val, has_pred) = self.parse_node_modifiers()?;
                Ok(Node {
                    kind: NodeKind::Context,
                    id: Some(id),
                    negated,
                    has_val,
                    has_pred,
                })
            }
            "x" | "t" => {
                self.expect_byte(b'(')?;
                let id = self.read_u32()?;
                self.expect_byte(b')')?;
                let (has_val, has_pred) = self.parse_node_modifiers()?;
                Ok(Node {
                    kind: NodeKind::ContextRef,
                    id: Some(id),
                    negated,
                    has_val,
                    has_pred,
                })
            }
            _ => {
                self.pos = start;
                Err(self.err(format!("expected node (N, N_, n, X, x, T, t), found '{ident}'")))
            }
        }
    }

    fn parse_node_modifiers(&mut self) -> Result<(bool, bool), ParseError> {
        let mut has_val = false;
        let mut has_pred = false;
        loop {
            self.skip_ws();
            if self.peek() == Some(b'.') {
                let save = self.pos;
                self.pos += 1;
                if self.try_word("val") {
                    self.expect_byte(b'(')?;
                    self.skip_balanced()?;
                    self.expect_byte(b')')?;
                    has_val = true;
                } else if self.try_word("test") {
                    self.expect_byte(b'(')?;
                    self.skip_balanced()?;
                    self.expect_byte(b')')?;
                    has_pred = true;
                } else {
                    self.pos = save;
                    break;
                }
            } else {
                break;
            }
        }
        Ok((has_val, has_pred))
    }

    fn parse_edge_spec(&mut self) -> Result<(bool, bool, bool), ParseError> {
        self.expect_byte(b'E')?;
        self.expect_byte(b'(')?;
        self.expect_byte(b')')?;
        let mut has_val = false;
        let mut has_pred = false;
        loop {
            self.skip_ws();
            if self.peek() == Some(b'.') {
                let save = self.pos;
                self.pos += 1;
                if self.try_word("val") {
                    self.expect_byte(b'(')?;
                    self.skip_balanced()?;
                    self.expect_byte(b')')?;
                    has_val = true;
                } else if self.try_word("test") {
                    self.expect_byte(b'(')?;
                    self.skip_balanced()?;
                    self.expect_byte(b')')?;
                    has_pred = true;
                } else {
                    self.pos = save;
                    break;
                }
            } else {
                break;
            }
        }
        Ok((false, has_val, has_pred))
    }

    fn parse_edge_op(&mut self) -> Option<Dir> {
        self.skip_ws();
        match self.peek() {
            Some(b'^') => {
                self.pos += 1;
                Some(Dir::Undirected)
            }
            Some(b'>') if self.peek2() == Some(b'>') => {
                self.pos += 2;
                Some(Dir::Forward)
            }
            Some(b'<') if self.peek2() == Some(b'<') => {
                self.pos += 2;
                Some(Dir::Backward)
            }
            Some(b'%') => {
                self.pos += 1;
                Some(Dir::Any)
            }
            _ => None,
        }
    }

    fn parse_stmt(&mut self) -> Result<Stmt, ParseError> {
        self.skip_ws();
        let negated = self.try_byte(b'!');
        let node = self.parse_node(negated)?;
        let edges = self.parse_edge_chain()?;
        Ok(Stmt { node, edges })
    }

    fn parse_edge_chain(&mut self) -> Result<Vec<Edge>, ParseError> {
        let mut edges = Vec::new();
        loop {
            self.skip_ws();
            let mut edge_negated = false;
            let mut has_edge_val = false;
            let mut has_edge_pred = false;

            if self.peek() == Some(b'&') {
                self.pos += 1;
                self.skip_ws();
                edge_negated = self.try_byte(b'!');
                let (_, val, pred) = self.parse_edge_spec()?;
                has_edge_val = val;
                has_edge_pred = pred;
            }

            match self.parse_edge_op() {
                Some(dir) => {
                    self.skip_ws();
                    let target_stmt = if self.peek() == Some(b'(') {
                        self.pos += 1;
                        let stmt = self.parse_stmt()?;
                        self.expect_byte(b')')?;
                        stmt
                    } else {
                        let target_negated = self.try_byte(b'!');
                        let target_node = self.parse_node(target_negated)?;
                        let target_edges = self.parse_edge_chain()?;
                        Stmt {
                            node: target_node,
                            edges: target_edges,
                        }
                    };
                    edges.push(Edge {
                        dir,
                        negated: edge_negated,
                        has_edge_val,
                        has_edge_pred,
                        target: target_stmt,
                    });
                    break;
                }
                None => break,
            }
        }
        Ok(edges)
    }

    fn parse_stmt_list(&mut self) -> Result<Vec<Stmt>, ParseError> {
        let mut stmts = Vec::new();
        self.skip_ws();
        if self.peek() == Some(b'}') {
            return Ok(stmts);
        }
        stmts.push(self.parse_stmt()?);
        loop {
            self.skip_ws();
            if self.peek() == Some(b'}') {
                break;
            }
            if self.try_byte(b',') {
                self.skip_ws();
                if self.peek() == Some(b'}') {
                    break;
                }
                stmts.push(self.parse_stmt()?);
            } else {
                break;
            }
        }
        Ok(stmts)
    }

    fn parse_cluster(&mut self) -> Result<Cluster, ParseError> {
        self.skip_ws();
        let decision = if self.try_word("get") {
            Decision::Get
        } else if self.try_word("ban") {
            Decision::Ban
        } else {
            return Err(self.err("expected 'get' or 'ban'"));
        };

        self.expect_byte(b'(')?;
        let morphism = self.parse_morphism()?;
        self.expect_byte(b')')?;
        self.expect_byte(b'{')?;
        let stmts = self.parse_stmt_list()?;
        self.expect_byte(b'}')?;

        Ok(Cluster {
            decision,
            morphism,
            stmts,
        })
    }

    fn skip_type_params(&mut self) {
        self.skip_ws();
        if self.peek() == Some(b'<') {
            self.pos += 1;
            let mut depth = 1i32;
            while depth > 0 {
                match self.advance() {
                    Some(b'<') => depth += 1,
                    Some(b'>') => depth -= 1,
                    None => break,
                    _ => {}
                }
            }
        }
        self.skip_ws();
        let _ = self.try_byte(b';');
    }

    fn strip_search_wrapper(&mut self) {
        self.skip_ws();
        let save = self.pos;
        if self.try_word("search") {
            self.skip_ws();
            if self.try_byte(b'!') {
                self.skip_ws();
                if self.try_byte(b'[') {
                    return;
                }
            }
        }
        self.pos = save;
    }

    pub fn parse(mut self) -> Result<Pattern, ParseError> {
        self.strip_search_wrapper();
        self.skip_type_params();

        let mut clusters = Vec::new();
        self.skip_ws();
        if self.at_end() || self.peek() == Some(b']') {
            return Ok(Pattern { clusters });
        }
        clusters.push(self.parse_cluster()?);
        loop {
            self.skip_ws();
            if self.at_end() || self.peek() == Some(b']') {
                break;
            }
            if self.try_byte(b',') {
                self.skip_ws();
                if self.at_end() || self.peek() == Some(b']') {
                    break;
                }
                clusters.push(self.parse_cluster()?);
            } else {
                break;
            }
        }
        Ok(Pattern { clusters })
    }
}

pub fn parse(input: &str) -> Result<Pattern, ParseError> {
    Parser::new(input).parse()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(input: &str) -> Pattern {
        parse(input).unwrap_or_else(|e| panic!("parse failed: {e}"))
    }

    #[test]
    fn single_edge() {
        let pat = p("get(Morphism::Mono) { N(0) ^ N(1) }");
        assert_eq!(pat.clusters.len(), 1);
        assert_eq!(pat.clusters[0].decision, Decision::Get);
        assert_eq!(pat.clusters[0].morphism, Morphism::Mono);
        assert_eq!(pat.clusters[0].stmts.len(), 1);
        let stmt = &pat.clusters[0].stmts[0];
        assert_eq!(stmt.node.kind, NodeKind::Free);
        assert_eq!(stmt.node.id, Some(0));
        assert_eq!(stmt.edges.len(), 1);
        assert_eq!(stmt.edges[0].dir, Dir::Undirected);
        assert_eq!(stmt.edges[0].target.node.id, Some(1));
    }

    #[test]
    fn triangle_with_refs() {
        let pat = p("get(Morphism::Iso) { N(0) ^ N(1), n(1) ^ N(2), n(0) ^ n(2) }");
        assert_eq!(pat.clusters[0].stmts.len(), 3);
        assert_eq!(pat.clusters[0].stmts[1].node.kind, NodeKind::FreeRef);
        assert_eq!(pat.clusters[0].stmts[2].edges[0].target.node.kind, NodeKind::FreeRef);
    }

    #[test]
    fn get_and_ban() {
        let pat = p(
            "get(Morphism::Mono) { N(0) ^ N(1) }, ban(Morphism::Mono) { n(0) ^ N(2), n(2) ^ n(1) }",
        );
        assert_eq!(pat.clusters.len(), 2);
        assert_eq!(pat.clusters[0].decision, Decision::Get);
        assert_eq!(pat.clusters[1].decision, Decision::Ban);
        assert_eq!(pat.clusters[1].stmts.len(), 2);
    }

    #[test]
    fn directed_forward() {
        let pat = p("get(Morphism::Mono) { N(0) >> N(1) }");
        assert_eq!(pat.clusters[0].stmts[0].edges[0].dir, Dir::Forward);
    }

    #[test]
    fn directed_backward() {
        let pat = p("get(Morphism::Mono) { N(0) << N(1) }");
        assert_eq!(pat.clusters[0].stmts[0].edges[0].dir, Dir::Backward);
    }

    #[test]
    fn negated_node() {
        let pat = p("get(Morphism::Mono) { N(0) ^ !N_() }");
        let target = &pat.clusters[0].stmts[0].edges[0].target;
        assert!(target.node.negated);
        assert_eq!(target.node.id, None);
    }

    #[test]
    fn context_node() {
        let pat = p("get(Morphism::Mono) { X(0) ^ N(1) }");
        assert_eq!(pat.clusters[0].stmts[0].node.kind, NodeKind::Context);
    }

    #[test]
    fn context_ref() {
        let pat = p("get(Morphism::Mono) { X(0) ^ N(1) }, ban(Morphism::Mono) { x(0) ^ N(2) }");
        assert_eq!(pat.clusters[1].stmts[0].node.kind, NodeKind::ContextRef);
    }

    #[test]
    fn edge_value() {
        let pat = p("get(Morphism::Mono) { N(0) & E().val(5) ^ N(1) }");
        let edge = &pat.clusters[0].stmts[0].edges[0];
        assert!(edge.has_edge_val);
        assert!(!edge.has_edge_pred);
        assert!(!edge.negated);
    }

    #[test]
    fn edge_pred() {
        let pat = p("get(Morphism::Mono) { N(0) & E().test(|e| e > 3) ^ N(1) }");
        let edge = &pat.clusters[0].stmts[0].edges[0];
        assert!(edge.has_edge_pred);
    }

    #[test]
    fn negated_edge() {
        let pat = p("get(Morphism::Mono) { N(0) & !E() ^ N(1) }");
        let edge = &pat.clusters[0].stmts[0].edges[0];
        assert!(edge.negated);
    }

    #[test]
    fn any_slot() {
        let pat = p("get(Morphism::Mono) { N(0) % N(1) }");
        assert_eq!(pat.clusters[0].stmts[0].edges[0].dir, Dir::Any);
    }

    #[test]
    fn type_params_stripped() {
        let pat = p("<(), ER> get(Morphism::Iso) { N(0) ^ N(1) }");
        assert_eq!(pat.clusters.len(), 1);
        assert_eq!(pat.clusters[0].morphism, Morphism::Iso);
    }

    #[test]
    fn chain() {
        let pat = p("get(Morphism::Mono) { N(0) ^ N(1) ^ N(2) }");
        let stmt = &pat.clusters[0].stmts[0];
        assert_eq!(stmt.edges.len(), 1);
        let target = &stmt.edges[0].target;
        assert_eq!(target.node.id, Some(1));
        assert_eq!(target.edges.len(), 1);
        assert_eq!(target.edges[0].target.node.id, Some(2));
    }

    #[test]
    fn search_wrapper_stripped() {
        let pat = p("search![\n  get(Morphism::Mono) { N(0) ^ N(1) }\n]");
        assert_eq!(pat.clusters.len(), 1);
    }

    #[test]
    fn search_wrapper_with_types_stripped() {
        let pat = p("search![<(), ER>;\n  get(Morphism::Mono) { N(0) ^ N(1) }\n]");
        assert_eq!(pat.clusters.len(), 1);
    }

    #[test]
    fn node_val() {
        let pat = p("get(Morphism::Mono) { N(0).val(42) ^ N(1) }");
        assert!(pat.clusters[0].stmts[0].node.has_val);
    }

    #[test]
    fn node_pred() {
        let pat = p("get(Morphism::Mono) { N(0).test(|n| n > 0) ^ N(1) }");
        assert!(pat.clusters[0].stmts[0].node.has_pred);
    }

    #[test]
    fn multiline() {
        let pat = p(
            r#"
            get(Morphism::Iso) {
                N(0) ^ N(1),
                n(1) ^ N(2),
                n(0) ^ n(2)
            }
            "#,
        );
        assert_eq!(pat.clusters[0].stmts.len(), 3);
    }

    #[test]
    fn morphism_without_prefix() {
        let pat = p("get(Iso) { N(0) ^ N(1) }");
        assert_eq!(pat.clusters[0].morphism, Morphism::Iso);
    }

    #[test]
    fn morphism_sub_iso() {
        let pat = p("get(Morphism::SubIso) { N(0) ^ N(1) }");
        assert_eq!(pat.clusters[0].morphism, Morphism::SubIso);
    }

    #[test]
    fn trailing_comma_in_stmts() {
        let pat = p("get(Morphism::Mono) { N(0) ^ N(1), }");
        assert_eq!(pat.clusters[0].stmts.len(), 1);
    }

    #[test]
    fn trailing_comma_in_clusters() {
        let pat = p("get(Morphism::Mono) { N(0) ^ N(1) },");
        assert_eq!(pat.clusters.len(), 1);
    }

    #[test]
    fn negated_edge_with_val() {
        let pat = p("get(Morphism::Mono) { N(0) & !E().val(7) ^ N(1) }");
        let edge = &pat.clusters[0].stmts[0].edges[0];
        assert!(edge.negated);
        assert!(edge.has_edge_val);
    }

    #[test]
    fn edge_val_and_pred() {
        let pat = p("get(Morphism::Mono) { N(0) & E().val(1).test(|e| true) ^ N(1) }");
        let edge = &pat.clusters[0].stmts[0].edges[0];
        assert!(edge.has_edge_val);
        assert!(edge.has_edge_pred);
    }

    #[test]
    fn any_slot_with_negated_edge() {
        let pat = p("get(Morphism::Mono) { N(0) & !E() % N(1) }");
        let edge = &pat.clusters[0].stmts[0].edges[0];
        assert!(edge.negated);
        assert_eq!(edge.dir, Dir::Any);
    }

    #[test]
    fn paren_grouped_target() {
        let pat = p("get(SubIso) { N(10) ^ (N(11) ^ N(12)) }");
        let stmt = &pat.clusters[0].stmts[0];
        assert_eq!(stmt.node.id, Some(10));
        assert_eq!(stmt.edges.len(), 1);
        let target = &stmt.edges[0].target;
        assert_eq!(target.node.id, Some(11));
        assert_eq!(target.edges.len(), 1);
        assert_eq!(target.edges[0].target.node.id, Some(12));
    }

    #[test]
    fn paren_grouped_with_ban() {
        let pat = p(
            r#"
            get(SubIso) {
                N(10) ^ (N(11) ^ N(12))
            },
            ban(Mono) {
                n(10) ^ N(20)
            }
            "#,
        );
        assert_eq!(pat.clusters.len(), 2);
        assert_eq!(pat.clusters[0].morphism, Morphism::SubIso);
        assert_eq!(pat.clusters[1].decision, Decision::Ban);
    }

    #[test]
    fn empty_pattern() {
        let pat = p("");
        assert_eq!(pat.clusters.len(), 0);
    }
}
