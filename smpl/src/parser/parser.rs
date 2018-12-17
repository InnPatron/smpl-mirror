use std::iter::{Iterator, Peekable};

use crate::{parser_state, parser_error};
use crate::span::*;
use crate::ast::*;
use super::tokens::*;
use super::expr_parser::*;
use super::parser_err::*;

pub type ParseErr<T> = Result<T, ParserError>;

#[macro_export]
macro_rules! consume_token  {

    ($input: expr, $state: expr) => {{
        use failure::Fail;
        use crate::parser::parser_err::*;
        use crate::parser_error;
        let next = $input.next()
            .ok_or(parser_error!(ParserErrorKind::UnexpectedEOI, $state))?
            .map_err(|e| parser_error!(ParserErrorKind::TokenizerError(e), $state))?;
        next.to_data()
    }};

    ($input: expr, $token: pat, $state: expr) => {{
        use failure::Fail;
        use crate::parser::parser_err::*;
        use crate::parser_error;
        let next = $input.next()
            .ok_or(parser_error!(ParserErrorKind::UnexpectedEOI, $state))?
            .map_err(|e| parser_error!(ParserErrorKind::TokenizerError(e), $state))?;
        let data = next.to_data();
        match data.1 {
            $token => data,
            _ => Err(parser_error!(ParserErrorKind::UnexpectedToken(data.1), $state))?,
        }
    }};

    ($input: expr, $token: pat => $e: expr, $state: expr) => {{
        use failure::Fail;
        use crate::parser::parser_err::*;
        use crate::parser_error;
        let next = $input.next()
            .ok_or(parser_error!(ParserErrorKind::UnexpectedEOI, $state))?
            .map_err(|e| parser_error!(ParserErrorKind::TokenizerError(e), $state))?;
        let data = next.to_data();
        match data.1 {
            $token => (data.0, $e),
            _ => Err(parser_error!(ParserErrorKind::UnexpectedToken(data.1), $state))?,
        }
    }};
}

#[macro_export]
macro_rules! peek_token {
    ($tokenizer: expr, $lam: expr, $state: expr) => {
        ($tokenizer).peek($lam).map_err(|e| parser_error!(e.into(), $state))?
    }
}

pub fn module(tokens: &mut BufferedTokenizer) -> ParseErr<Module> {

    enum ModDec {
        Struct,
        Annotation,
        Function(bool),
        Use,
        Err,
    }

    let mut name = None;
    if peek_token!(tokens, |tok| {
        match tok {
            Token::Mod => true,
            _ => false,
        }
    }, parser_state!("module", "mod-decl")) {
        // Found mod declaration
        name = Some(module_decl(tokens)?);
    }

    let mut decls = Vec::new();
    let mut anno = Vec::new();

    while tokens.has_next() {
        match peek_token!(tokens, |tok| {
            match tok {
                Token::Struct => ModDec::Struct,
                Token::Pound => ModDec::Annotation,
                Token::Fn => ModDec::Function(false),
                Token::Builtin => ModDec::Function(true),
                Token::Use => ModDec::Use,
                _ => ModDec::Err,
            }
        }, parser_state!("module", "decl-kind")) {
            ModDec::Struct => {
                decls.push(DeclStmt::Struct(struct_decl(tokens, anno)?));
                anno = Vec::new();
            }

            ModDec::Annotation => {
                anno = annotations(tokens)?;
            },

            ModDec::Function(is_builtin) => {
                decls.push(fn_decl(tokens, anno, is_builtin)?);
                anno = Vec::new();
            }

            ModDec::Use => {
                decls.push(use_decl(tokens)?);
                anno = Vec::new();
            }

            ModDec::Err => unimplemented!("Unexpected token: {:?}", tokens.next().unwrap()),
        }
    }

    let module = Module(name, decls);

    Ok(module)
}

fn annotations(tokens: &mut BufferedTokenizer) -> ParseErr<Vec<Annotation>> {
    let mut annotations = Vec::new();

    while tokens.has_next() && peek_token!(tokens, |tok| {
        match tok {
            Token::Pound => true,
            _ => false,
        }
    }, parser_state!("annotations", "more-annotation-indication")) {
        let _pound = consume_token!(tokens, 
                                    Token::Pound,
                                    parser_state!("annotations", "pound"));
        let _lbracket = consume_token!(tokens, 
                                       Token::LBracket,
                                       parser_state!("annotations", "lbracket"));
        annotations.push(Annotation{ keys: kv_list(tokens)? });
        let _rbracket = consume_token!(tokens, 
                                       Token::RBracket,
                                       parser_state!("annotations", "rbracket"));
    }

    Ok(annotations)
}

fn kv_list(tokens: &mut BufferedTokenizer) -> ParseErr<Vec<(Ident, Option<String>)>> {
    let mut list = vec![kv_pair(tokens)?];

    while tokens.has_next() {
        if peek_token!(tokens, |tok| {
            match tok {
                Token::Comma => true,
                _ => false
            }
        }, parser_state!("kv-list", "comma-separator")) {
            let _comma = consume_token!(tokens, 
                                        Token::Comma,
                                        parser_state!("kv-list", "comma-separator"));
            if tokens.has_next() && peek_token!(tokens, |tok| {
                match tok {
                    Token::RBracket => false,
                    _ => true,
                }
            }, parser_state!("kv-list", "rbracket")) {
                list.push(kv_pair(tokens)?);
                continue;
            }
        }

        break;
    }

    Ok(list)
}

fn kv_pair(tokens: &mut BufferedTokenizer) -> ParseErr<(Ident, Option<String>)> {
    let (_, ident) = consume_token!(tokens, 
                                    Token::Identifier(i) => i, 
                                    parser_state!("kvpair", "key"));

    if peek_token!(tokens, |tok| {
        match tok {
            Token::Assign => true,
            _ => false,
        }
    }, parser_state!("kv-pair", "=")) {
        let _assign = consume_token!(tokens, 
                                     Token::Assign,
                                     parser_state!("kvpair", "assign"));
        let (_, v) = consume_token!(tokens, 
                                    Token::StringLiteral(s) => s,
                                    parser_state!("kvpair", "value"));
        Ok((Ident(ident), Some(v)))
    } else {
        Ok((Ident(ident), None))
    }
}

fn use_decl(tokens: &mut BufferedTokenizer) -> ParseErr<DeclStmt> {
    let (uspan, _) = consume_token!(tokens, 
                                    Token::Use,
                                    parser_state!("use-decl", "use"));
    let (mspan, module) = consume_token!(tokens, 
                                         Token::Identifier(i) => Ident(i),
                                         parser_state!("use-decl", "name"));
    let _semi = consume_token!(tokens, 
                               Token::Semi,
                               parser_state!("use-decl", "semicolon"));

    let span = LocationSpan::new(uspan.start(), mspan.end());

    let use_decl = UseDecl(AstNode::new(module, mspan));

    let use_decl = DeclStmt::Use(AstNode::new(use_decl, span));
    
    Ok(use_decl)
}

#[cfg(test)]
pub fn testfn_decl(tokens: &mut BufferedTokenizer) -> ParseErr<Function> {
    let decl = fn_decl(tokens, vec![], false)?;
    match decl {
        DeclStmt::Function(f) => Ok(f.to_data().0),
        _ => unreachable!()
    }
}

fn fn_decl(tokens: &mut BufferedTokenizer, annotations: Vec<Annotation>, is_builtin: bool) -> ParseErr<DeclStmt> {
    let mut span = Span::dummy();
    if is_builtin {
        let (bloc, _builtin) = consume_token!(tokens, 
                                              Token::Builtin,
                                              parser_state!("fn-decl", "builtin"));
        span = bloc;
    }

    let (fnloc, _) = consume_token!(tokens, 
                                    Token::Fn,
                                    parser_state!("fn-decl", "fn"));
    if !is_builtin {
        span = fnloc;
    }

    let (idloc, ident) = consume_token!(tokens, 
                                        Token::Identifier(i) => Ident(i),
                                        parser_state!("fn-decl", "name"));
    let _lparen = consume_token!(tokens, 
                                 Token::LParen,
                                 parser_state!("fn-decl", "parameter lparen"));

    let params = if peek_token!(tokens, |tok| {
        match tok {
            Token::Unchecked => true,
            _ => false
        }
    }, parser_state!("fn-decl", "UNCHECKED paramter")) {
        let _unchecked = consume_token!(tokens, 
                                        Token::Unchecked,
                                        parser_state!("fn-decl", "UNCHECKED parameter"));
        BuiltinFnParams::Unchecked
    } else {
        if peek_token!(tokens, |tok| {
            match tok {
                Token::RParen => false,
                _ => true,
            }
        }, parser_state!("fn-decl", "rparen")) {
            BuiltinFnParams::Checked(Some(fn_param_list(tokens)?))
        } else {
            BuiltinFnParams::Checked(None)
        }
    };
        
    let (rloc, _) = consume_token!(tokens, 
                                   Token::RParen,
                                   parser_state!("fn-decl", "parameter rparen"));
    span = Span::combine(span, rloc);

    let mut return_type = None;
    if peek_token!(tokens, |tok| {
        match tok {
            Token::Arrow => true,
            _ => false,
        }
    }, parser_state!("fn-decl", "return type arrow?")) {
        let _arrow = consume_token!(tokens, 
                                    Token::Arrow,
                                    parser_state!("fn-decl", "return type arrow"));
        return_type = Some(type_annotation(tokens)?);
    }

    let mut body: Option<AstNode<Block>> = None;
    if !is_builtin {
        body = Some(block(tokens)?);
    }

    if is_builtin {
        let (semiloc, _) = consume_token!(tokens, 
                                          Token::Semi,
                                          parser_state!("fn-decl", "builtin-semicolon"));
        span = Span::combine(span, semiloc);
    }

    if is_builtin {
        Ok(DeclStmt::BuiltinFunction(
            AstNode::new(
                BuiltinFunction {
                    name: AstNode::new(ident, idloc),
                    params: params,
                    return_type: return_type,
                    annotations: annotations,
                },
                span)
            )
        )
    } else {
        let params = match params {
            BuiltinFnParams::Unchecked => 
                return Err(parser_error!(
                        ParserErrorKind::NonbuiltinUncheckedParameters,
                        parser_state!("fn-decl", "param-validation"))),
            BuiltinFnParams::Checked(p) => p,
        };

        let body = match body {
            Some(b) => b,
            None => return Err(parser_error!(
                    ParserErrorKind::NoFnBody,
                    parser_state!("fn-decl", "body"))),
        };

        Ok(DeclStmt::Function(
            AstNode::new(
                Function {
                    name: AstNode::new(ident, idloc),
                    params: params,
                    return_type: return_type,
                    body: body,
                    annotations: annotations,
                },
                span)
            )
        )
    }
}

pub fn fn_param_list(tokens: &mut BufferedTokenizer) -> ParseErr<Vec<AstNode<FnParameter>>> {
    let mut list = vec![fn_param(tokens)?];

    while tokens.has_next() {
        if peek_token!(tokens, |tok| {
            match tok {
                Token::Comma => true,
                _ => false
            }
        }, parser_state!("fn-param-list", "comma separator")) {
            let _comma = consume_token!(tokens, 
                                        Token::Comma,
                                        parser_state!("fn-param-list", "comma separator"));
            if tokens.has_next() && peek_token!(tokens, |tok| {
                match tok {
                    Token::RParen => false,
                    _ => true,
                }
            }, parser_state!("fn-param-list", "rparen")) {
                list.push(fn_param(tokens)?);
                continue;
            }
        }

        break;
    }

    Ok(list)
}

fn fn_param(tokens: &mut BufferedTokenizer) -> ParseErr<AstNode<FnParameter>> {
    let (idloc, ident) = consume_token!(tokens, 
                                        Token::Identifier(i) => Ident(i),
                                        parser_state!("fn-param", "parameter name"));
    let _colon = consume_token!(tokens, 
                                Token::Colon,
                                parser_state!("fn-param", "param type colon"));
    let ann = type_annotation(tokens)?;

    let span = Span::combine(idloc, ann.span());
    let param = FnParameter {
        name: AstNode::new(ident, idloc),
        param_type: ann,
    };

    Ok(AstNode::new(param, span))
}

#[cfg(test)]
pub fn teststruct_decl(tokens: &mut BufferedTokenizer) -> ParseErr<Struct> {
    let decl = struct_decl(tokens, vec![])?.to_data().0;
    Ok(decl)
}

fn struct_decl(tokens: &mut BufferedTokenizer, anns: Vec<Annotation>) -> ParseErr<AstNode<Struct>> {

    let (structLoc, _) = consume_token!(tokens, 
                                        Token::Struct,
                                        parser_state!("struct-decl", "struct"));
    let (nameLoc, structName) = consume_token!(tokens, 
                                               Token::Identifier(i) => Ident(i),
                                               parser_state!("struct-decl", "name"));
    let _lbrace = consume_token!(tokens, 
                                 Token::LBrace,
                                 parser_state!("struct-decl", "fields lbrace"));

    // Check if no struct fields
    let body = if peek_token!(tokens, |tok| {
        match tok {
            Token::RBrace => false,
            _ => true,
        }
    }, parser_state!("struct-decl", "fields rbrace")) {
        let body = struct_field_list(tokens)?;
        StructBody(Some(body))
    } else {
        // Empty struct body
        StructBody(None)
    };

    // Get Keys
    let (rloc, _) = consume_token!(tokens, 
                                   Token::RBrace,
                                   parser_state!("struct-decl", "fields rbrace"));

    let overallSpan = LocationSpan::new(structLoc.start(), rloc.start());

    Ok(AstNode::new(Struct {
            name: AstNode::new(structName, nameLoc),
            body: body,
            annotations: anns,
        }, overallSpan)
    )
}

fn struct_field_list(tokens: &mut BufferedTokenizer) -> ParseErr<Vec<StructField>> {
    let mut list = vec![struct_field(tokens)?];

    while tokens.has_next() {
        if peek_token!(tokens, |tok| {
            match tok {
                Token::Comma => true,
                _ => false
            }
        }, parser_state!("struct-field-list", "comma separator")) {
            let _comma = consume_token!(tokens, 
                                        Token::Comma,
                                        parser_state!("struct-field-list", "comma separator"));
            if tokens.has_next() && peek_token!(tokens, |tok| {
                match tok {
                    Token::RBrace => false,
                    _ => true,
                }
            }, parser_state!("struct-field-list", "rbrace")) {
                list.push(struct_field(tokens)?);
                continue;
            }
        }

        break;
    }

    Ok(list)
}


fn struct_field(tokens: &mut BufferedTokenizer) -> ParseErr<StructField> {
    let (idloc, ident) = consume_token!(tokens, 
                                        Token::Identifier(i) => Ident(i),
                                        parser_state!("struct-field", "name"));
    let _colon = consume_token!(tokens, 
                                Token::Colon,
                                parser_state!("struct-field", "field type colon"));
    let ann = type_annotation(tokens)?;

    Ok(StructField {
        name: AstNode::new(ident, idloc),
        field_type: ann,
    })
}

fn module_decl(tokens: &mut BufferedTokenizer) -> ParseErr<AstNode<Ident>> {
    // Consume MOD
    let (modloc, _) = consume_token!(tokens, 
                                     Token::Mod,
                                     parser_state!("mod-decl", "mod"));
    let (_idloc, ident) = consume_token!(tokens, 
                                         Token::Identifier(i) => Ident(i),
                                         parser_state!("mod-decl", "name"));
    let (semiloc, _) = consume_token!(tokens, 
                                      Token::Semi,
                                      parser_state!("mod-decl", "semicolon"));

    let span = LocationSpan::new(modloc.start(), semiloc.end());
    Ok(AstNode::new(ident, span))
}

pub fn type_annotation(tokens: &mut BufferedTokenizer) -> ParseErr<AstNode<TypeAnnotation>> {
    
    enum TypeAnnDec {
        Module,
        FnType,
        ArrayType,
        Err,
    }

    if tokens.has_next() == false {
        unimplemented!("Unexpected End of Input: Expected Type Annotation");
    }
    match peek_token!(tokens, |tok| {
        match tok {
            Token::Fn => TypeAnnDec::FnType,
            Token::Identifier(_) => TypeAnnDec::Module,
            Token::LBracket => TypeAnnDec::ArrayType,

            _ => TypeAnnDec::Err,
        }

    }, parser_state!("type-annotation", "annotation-kind")) {
        TypeAnnDec::Module => {
            let (bind, span) = module_binding(tokens)?.to_data();

            Ok(AstNode::new(TypeAnnotation::Path(bind), span))
        },
        TypeAnnDec::FnType => fn_type(tokens),
        TypeAnnDec::ArrayType => array_type(tokens),

        TypeAnnDec::Err => unimplemented!(),
    }
}

pub fn module_binding(tokens: &mut BufferedTokenizer) -> ParseErr<AstNode<ModulePath>> {
    let mut path = Vec::new();
    let (floc, first) =  consume_token!(tokens, 
                                        Token::Identifier(i) => Ident(i),
                                        parser_state!("module-binding", "root"));

    let mut binding_span = LocationSpan::new(floc.start(), floc.end());
    path.push(AstNode::new(first, floc));

    if peek_token!(tokens, |tok| {
        match tok {
            Token::ColonColon => true,
            _ => false,
        }
    }, parser_state!("module-binding", "segment coloncolon")) {
        let _coloncolon = consume_token!(tokens, 
                                         Token::ColonColon,
                                         parser_state!("module-binding", "segment coloncolon"));
        let (nloc, next) = consume_token!(tokens, 
                                          Token::Identifier(i) => Ident(i),
                                          parser_state!("module-binding", "segment name"));
        path.push(AstNode::new(next, nloc));
        binding_span = LocationSpan::new(floc.start(), nloc.end());
    }

    Ok(AstNode::new(ModulePath(path), binding_span))
}

fn array_type(tokens: &mut BufferedTokenizer) -> ParseErr<AstNode<TypeAnnotation>> {
    let (lloc, _) = consume_token!(tokens, 
                                   Token::LBracket,
                                   parser_state!("array-type", "lbracket"));
    let base_type = Box::new(type_annotation(tokens)?);
    let _semi = consume_token!(tokens, 
                               Token::Semi,
                               parser_state!("array-type", "semicolon"));
    let (_, number) = consume_token!(tokens, 
                                     Token::IntLiteral(i) => i,
                                     parser_state!("array-type", "array size"));
    let (rloc, _) = consume_token!(tokens, 
                                   Token::RBracket,
                                   parser_state!("array-type", "rbracket"));

    let array_type_span = LocationSpan::new(lloc.start(), rloc.end());

    if number <= 0 {
        unimplemented!("Parser error: number of elements must be greater than 0. Found {}", number);
    }

    Ok(AstNode::new(TypeAnnotation::Array(base_type, number as u64), 
                    array_type_span))
}

fn fn_type(tokens: &mut BufferedTokenizer) -> ParseErr<AstNode<TypeAnnotation>> {
    let (fnloc, _) = consume_token!(tokens, 
                                    Token::Fn,
                                    parser_state!("fn-type", "fn"));
    let _lparen = consume_token!(tokens, 
                                 Token::LParen,
                                 parser_state!("fn-type", "param lparen"));
        
    let mut params = None;
    if tokens.has_next() && peek_token!(tokens, |tok| {
        match tok {
            Token::RParen => false,
            _ => true,
        }
    }, parser_state!("fn-type", "param rparen")) {
        params = Some(fn_type_params(tokens)?);
    }

    let (rparenloc, _) = consume_token!(tokens, 
                                        Token::RParen,
                                        parser_state!("fn-type", "param rparen"));

    let mut fn_type_span = LocationSpan::new(fnloc.start(), rparenloc.end());

    let mut return_type = None;
    if tokens.has_next() && peek_token!(tokens, |tok| {
        match tok {
            Token::Arrow => true,
            _ => false,
        }
    }, parser_state!("fn-type", "return type arrow")) {
        let _arrow = consume_token!(tokens, 
                                    Token::Arrow,
                                    parser_state!("fn-type", "return type arrow"));
        let ret = type_annotation(tokens)?;
        let return_span = ret.span();

        return_type = Some(Box::new(ret));
        fn_type_span = Span::combine(fn_type_span, return_span);
    }

    Ok(AstNode::new(TypeAnnotation::FnType(params, return_type), fn_type_span))
}

fn fn_type_params(tokens: &mut BufferedTokenizer) -> ParseErr<Vec<AstNode<TypeAnnotation>>> {
    let mut list = vec![type_annotation(tokens)?];

    while tokens.has_next() {
        if peek_token!(tokens, |tok| {
            match tok {
                Token::Comma => true,
                _ => false
            }
        }, parser_state!("fn-type-params", "comma separator")) {
            let _comma = consume_token!(tokens, 
                                        Token::Comma,
                                        parser_state!("fn-type-params", "comma separator"));
            if tokens.has_next() && peek_token!(tokens, |tok| {
                match tok {
                    Token::RParen => false,
                    _ => true,
                }
            }, parser_state!("fn-type-params", "rparen")) {
                list.push(type_annotation(tokens)?);
                continue;
            }
        }

        break;
    }

    Ok(list)
}

pub fn block(tokens: &mut BufferedTokenizer) -> ParseErr<AstNode<Block>> {


    let (lloc, _) = consume_token!(tokens, 
                                   Token::LBrace,
                                   parser_state!("block", "lbrace"));

    let mut stmts = Vec::new();
    // Parse for all statements untile '}'
    while tokens.has_next() && peek_token!(tokens, |tok| {
        match tok {
            Token::RBrace => false,
            _ => true,
        }
    }, parser_state!("block", "rbrace")) {
        stmts.push(stmt(tokens)?);
    }
    
    
    let (rloc, _) = consume_token!(tokens, 
                                   Token::RBrace,
                                   parser_state!("block", "rbrace"));

    let span = LocationSpan::new(lloc.start(), rloc.end());

    let block = Block(stmts);

    Ok(AstNode::new(
        block,
        span)
    )
}

#[cfg(test)]
pub fn teststmt(tokens: &mut BufferedTokenizer) -> ParseErr<Stmt> {
    stmt(tokens)
}


fn stmt(tokens: &mut BufferedTokenizer) -> ParseErr<Stmt> {
    enum StmtDec {
        Continue,
        Break,
        Return,

        While,
        If,

        LocalVar,
        PotentialAssign,

        Expr,
    }

    let stmt = match peek_token!(tokens, |tok| {
        match tok {
            Token::Continue => StmtDec::Continue,
            Token::Break => StmtDec::Break,
            Token::Return => StmtDec::Return,

            Token::While => StmtDec::While,
            Token::If => StmtDec::If,

            Token::Let => StmtDec::LocalVar,
            
            Token::Identifier(_) => StmtDec::PotentialAssign,

            _ => StmtDec::Expr,
        }
    }, parser_state!("stmt", "stmt kind")) {

        StmtDec::Continue => {
            Stmt::ExprStmt(continue_stmt(tokens)?)
        }

        StmtDec::Break => {
            Stmt::ExprStmt(break_stmt(tokens)?)
        }

        StmtDec::Return => {
            Stmt::ExprStmt(return_stmt(tokens)?)
        }

        StmtDec::While => {
            Stmt::ExprStmt(while_stmt(tokens)?)
        }

        StmtDec::If => {
            Stmt::ExprStmt(if_stmt(tokens)?)
        }

        StmtDec::LocalVar => {
            Stmt::ExprStmt(local_var_decl(tokens)?)
        }

        StmtDec::PotentialAssign => {
            potential_assign(tokens)?
        }

        StmtDec::Expr => {
            let expr = piped_expr(tokens, &[Delimiter::Semi])?;

            let _semi = consume_token!(tokens, 
                                       Token::Semi,
                                       parser_state!("stmt-expr", "semicolon"));

            Stmt::Expr(expr)
        }
    };

    Ok(stmt)
}

fn potential_assign(tokens: &mut BufferedTokenizer) -> ParseErr<Stmt> {
    enum Dec {
        AccessPath,
        ModulePath,
        FnCall,
        Indexing,
        DefSingletonAssignment,
        DefSingletonExpr,
    }

    enum PathDec {
        Assign,
        Expr,
    }

    let (base_span, base_ident) = consume_token!(tokens,
                                                 Token::Identifier(ident) => Ident(ident),
                                                 parser_state!("potential-assign", "root"));

    // Check if there is a full Access Path
    // If there is module path or function call, parse for expr ';'
    // If there is a 'ident = ...', parse for assignment
    // If there is a 'ident op ...', parse for expr ';'
    let path = match peek_token!(tokens, |tok| {
        match tok {
            Token::Dot => Dec::AccessPath,
            Token::ColonColon => Dec::ModulePath,
            Token::LParen => Dec::FnCall,
            Token::LBracket => Dec::Indexing,
            Token::Assign => Dec::DefSingletonAssignment,
            _ => Dec::DefSingletonExpr,
        }
    }, parser_state!("potential-assign", "lhs kind")) {

        Dec::AccessPath => {
            let span = base_span;
            let root = PathSegment::Ident(AstNode::new(base_ident, span));
            
            let (path, _span) = access_path(tokens, root)?.to_data();

            match path {
                Expr::FieldAccess(path) => path,
                _ => unreachable!(),
            }
        }

        Dec::ModulePath => {
            // Expecting: expr ';'
            // Module paths are not lvalues

            let path = expr_module_path(tokens, base_ident, base_span)?;
            let expr = prebase_piped_expr(tokens, path, &[Delimiter::Semi])?;

            let _semi = consume_token!(tokens, 
                                       Token::Semi,
                                       parser_state!("stmt-expr-module-path", "semicolon"));

            return Ok(Stmt::Expr(expr));
        }

        Dec::FnCall => {
            // Expecting: expr ';'
            // Function calls are not lvalues

            let args = fn_args(tokens)?;
            let (args, arg_span) = args.to_data();
            let args = args.map(|v| v.into_iter().map(|a| a.to_data().0).collect());

            let called = ModulePath(vec![AstNode::new(base_ident, base_span)]);

            let fn_call = FnCall {
                path: called,
                args: args,
            };


            let span = Span::combine(base_span, arg_span);
            let expr_base = AstNode::new(fn_call, span);
            let expr_base = AstNode::new(Expr::FnCall(expr_base), span);

            let expr = prebase_piped_expr(tokens, expr_base, &[Delimiter::Semi])?;

            let _semi = consume_token!(tokens, 
                                       Token::Semi,
                                       parser_state!("stmt-expr-fn-call", "semicolon"));
            
            return Ok(Stmt::Expr(expr));
        }

        Dec::Indexing => {
            let _lbracket = consume_token!(tokens, 
                                           Token::LBracket,
                                           parser_state!("potential_assign-indexing", "lbracket"));
            let indexer = piped_expr(tokens, &[Delimiter::RBracket])?;
            let (indexer, _) = indexer.to_data();
            let (_rspan, _rbracket) = consume_token!(tokens, 
                                                     Token::RBracket,
                                                     parser_state!("potential_assign-indexing", "rbracket"));

            if peek_token!(tokens, |tok| {
                match tok {
                    Token::Dot => true,
                    _ => false,
                }
            }, parser_state!("potential-assign", "is-access-path")) {

                // Access path with indexing as root
                let span = base_span;
                let root = PathSegment::Indexing(AstNode::new(base_ident, span), Box::new(indexer));

                let (path, _span) = access_path(tokens, root)?.to_data();
                match path {
                    Expr::FieldAccess(path) => path,
                    _ => unreachable!(),
                }

            } else {

                //Single indexing
                let span = base_span;
                let root = PathSegment::Indexing(AstNode::new(base_ident, span), Box::new(indexer));

                let path = vec![root];
                let path = Path(path);
                
                AstNode::new(path, span)
            }
        },

        Dec::DefSingletonAssignment => {
            let _assignop = consume_token!(tokens, 
                                           Token::Assign,
                                           parser_state!("assignment", "="));

            let value = piped_expr(tokens, &[Delimiter::Semi])?;
            let (value, value_span) = value.to_data();

            let _semi = consume_token!(tokens, 
                                       Token::Semi,
                                       parser_state!("assignment", "semicolon"));

            let segment = PathSegment::Ident(AstNode::new(base_ident, base_span));
            let path = vec![segment];
            let path = AstNode::new(Path(path), base_span);

            let assignment_span = Span::combine(base_span, value_span);
            let assignment = Assignment {
                name: path,
                value: value,
            };

            let assignment = ExprStmt::Assignment(assignment);

            return Ok(Stmt::ExprStmt(AstNode::new(assignment, assignment_span)));
        }

        Dec::DefSingletonExpr => {
            // Expecting: expr ';'
            // 'ident op' is not a lvalue
            let _span = base_span;
            let expr = piped_expr(tokens, &[Delimiter::Semi])?;

            let _semi = consume_token!(tokens, 
                                       Token::Semi,
                                       parser_state!("expr-stmt-singleton", "semicolon"));

            return Ok(Stmt::Expr(expr));
        }
    };

    // Found a full path
    // Check if it's an assignment or expression

    if tokens.has_next() == false {
        return Err(parser_error!(
                ParserErrorKind::UnexpectedEOI.into(),
                parser_state!("full-path-potential-assign", "=")));
    }

    match peek_token!(tokens, |tok| {
        match tok {
            Token::Assign => PathDec::Assign,
            _ => PathDec::Expr,
        }
    }, parser_state!("full-path-potential-assign", "=;")) {
        
        PathDec::Assign => {
            let path = path;

            let _assign = consume_token!(tokens, 
                                         Token::Assign,
                                         parser_state!("assignment", "="));

            let (value, value_span) = piped_expr(tokens, &[Delimiter::Semi])?.to_data();

            let _semi = consume_token!(tokens, 
                                       Token::Semi,
                                       parser_state!("assignment", "semicolon"));

            let span = Span::combine(path.span(), value_span);
            let assignment = Assignment {
                name: path,
                value: value,
            };

            Ok(Stmt::ExprStmt(AstNode::new(ExprStmt::Assignment(assignment), span)))
        }

        PathDec::Expr => {
            let _span = path.span();
            let _path = Expr::FieldAccess(path);

            let expr = piped_expr(tokens, &[Delimiter::Semi])?;

            let _semi = consume_token!(tokens, 
                                       Token::Semi,
                                       parser_state!("stmt-expr-path", "semicolon"));

            Ok(Stmt::Expr(expr))
        }
    }
}

fn local_var_decl(tokens: &mut BufferedTokenizer) -> ParseErr<AstNode<ExprStmt>> {
    let (letloc, _) = consume_token!(tokens, 
                                     Token::Let,
                                     parser_state!("local-var-decl", "let"));

    let (iloc, ident) = consume_token!(tokens, 
                                       Token::Identifier(i) => Ident(i),
                                       parser_state!("local-var-decl", "name"));
    let ident = AstNode::new(ident, iloc);

    let mut type_anno = None;

    if tokens.has_next() == false {
        return Err(parser_error!(
                ParserErrorKind::UnexpectedEOI.into(),
                parser_state!("local-var-decl", "=")));
    }

    if peek_token!(tokens, |tok| {
        match tok {
            Token::Colon => true,
            _ => false,
        }
    }, parser_state!("local-var-decl", "type colon")) {

        let _colon = consume_token!(tokens, 
                                    Token::Colon,
                                    parser_state!("local-var-decl", "type colon"));
        type_anno = Some(type_annotation(tokens)?);
    }

    let _assign = consume_token!(tokens, 
                                 Token::Assign,
                                 parser_state!("local-var-decl", "="));

    let init_value = piped_expr(tokens, &[Delimiter::Semi])?.to_data();

    let (semiloc, _) = consume_token!(tokens, 
                                      Token::Semi,
                                      parser_state!("local-var-decl", "semicolon"));

    let span = LocationSpan::new(letloc.start(), semiloc.end());

    let local_var_decl = LocalVarDecl {
        var_type: type_anno,
        var_name: ident,
        var_init: init_value.0,
    };

    Ok(AstNode::new(ExprStmt::LocalVarDecl(local_var_decl), span))
}

fn if_stmt(tokens: &mut BufferedTokenizer) -> ParseErr<AstNode<ExprStmt>> {

    enum IfDec {
        Elif,
        Else,
        End,
    }

    let (ifloc, _) = consume_token!(tokens, 
                                    Token::If,
                                    parser_state!("if-stmt", "if"));

    let mut end = ifloc;

    let first_branch = if_branch(tokens)?;
    end = first_branch.block.span();

    let mut branches = vec![first_branch];
    let mut default_branch = None;

    while tokens.has_next() {
        match peek_token!(tokens, |tok| {
            match tok {
                Token::Elif => IfDec::Elif,
                Token::Else => IfDec::Else,

                _ => IfDec::End,
            }
        }, parser_state!("if-stmt", "branches")) {
            IfDec::Elif => {
                let _elif = consume_token!(tokens, 
                                           Token::Elif,
                                           parser_state!("if-stmt", "elif"));

                let branch = if_branch(tokens)?;
                end = branch.block.span();

                branches.push(branch);
            }

            IfDec::Else => {
                let _else = consume_token!(tokens, 
                                           Token::Else,
                                           parser_state!("if-stmt", "else"));
                let block = block(tokens)?;

                end = block.span();
                default_branch = Some(block);

                break;
            }

            IfDec::End => break,
        }
    }

    let span = Span::combine(ifloc, end);

    let if_stmt = If {
        branches: branches,
        default_block: default_branch,
    };

    Ok(AstNode::new(ExprStmt::If(if_stmt), span))
}

fn if_branch(tokens: &mut BufferedTokenizer) -> ParseErr<Branch> {
    let conditional = piped_expr(tokens, &[Delimiter::LBrace])?;

    let block = block(tokens)?;

    let branch = Branch {
        conditional: conditional,
        block: block,
    };

    Ok(branch)
}

fn while_stmt(tokens: &mut BufferedTokenizer) -> ParseErr<AstNode<ExprStmt>> {
    let (whileloc, _) = consume_token!(tokens, 
                                       Token::While,
                                       parser_state!("while-stmt", "while"));

    let conditional = piped_expr(tokens, &[Delimiter::LBrace])?;

    let block = block(tokens)?;

    let span = Span::combine(whileloc, block.span());

    let while_stmt = While {
        conditional: conditional,
        block: block,
    };

    Ok(AstNode::new(ExprStmt::While(while_stmt), span))
}

fn return_stmt(tokens: &mut BufferedTokenizer) -> ParseErr<AstNode<ExprStmt>> {
    let (returnloc, _) = consume_token!(tokens, 
                                        Token::Return,
                                        parser_state!("return-stmt", "return"));

    let mut end = returnloc;

    let expr = if peek_token!(tokens, |tok| {
        match tok {
            Token::Semi => true,
            _ => false,
        }
    }, parser_state!("return-stmt", "semi-colon")) {
        // No expression
        let (semiloc, _) = consume_token!(tokens, 
                                          Token::Semi,
                                          parser_state!("return-stmt", "semicolon"));
        end = semiloc;

        None
    } else {
        // Expression
        let expr = piped_expr(tokens, &[Delimiter::Semi])?;

        let _semi = consume_token!(tokens, 
                                   Token::Semi,
                                   parser_state!("return-stmt", "semicolon"));

        end = expr.span();

        Some(expr)
    };

    let span = Span::combine(returnloc, end);
    Ok(AstNode::new(ExprStmt::Return(span, expr.map(|e| e.to_data().0)), span))
}

fn continue_stmt(tokens: &mut BufferedTokenizer) -> ParseErr<AstNode<ExprStmt>> {
    let (contloc, _) = consume_token!(tokens, 
                                      Token::Continue,
                                      parser_state!("continue-stmt", "continue"));
    let (semiloc, _) = consume_token!(tokens, 
                                      Token::Semi,
                                      parser_state!("continue-stmt", "semicolon"));

    let span = LocationSpan::new(contloc.start(), semiloc.end());
    Ok(AstNode::new(
        ExprStmt::Continue(span),
        span)
    )
}

fn break_stmt(tokens: &mut BufferedTokenizer) -> ParseErr<AstNode<ExprStmt>> {
    let (contloc, _) = consume_token!(tokens, 
                                      Token::Break, 
                                      parser_state!("break-stmt", "break"));
    let (semiloc, _) = consume_token!(tokens, 
                                      Token::Semi,
                                      parser_state!("break-stmt", "semicolon"));

    let span = LocationSpan::new(contloc.start(), semiloc.end());
    Ok(AstNode::new(
        ExprStmt::Break(span),
        span)
    )
}
