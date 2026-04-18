//! Tiny hand-rolled HTML templates. Kept inline (no template engine) because
//! the web server only has two forms and shipping a crate like `askama` or
//! `minijinja` just to render ~40 lines of markup isn't worth the dep weight.

use std::fmt::Write;

const BASE_STYLE: &str = r#"
* { box-sizing: border-box; }
body {
  margin: 0;
  background: linear-gradient(180deg, #0e0e12 0%, #1a1420 100%);
  color: #e8e4dc;
  font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
  min-height: 100vh;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 2rem 1rem;
}
.card {
  background: rgba(26, 20, 32, 0.92);
  border: 1px solid #3a2f4a;
  border-radius: 10px;
  box-shadow: 0 8px 32px rgba(0,0,0,0.4);
  padding: 2rem 2.25rem;
  width: 100%;
  max-width: 420px;
}
h1 {
  font-size: 1.35rem;
  margin: 0 0 0.25rem;
  letter-spacing: 0.02em;
}
.subtitle { color: #9a93a8; font-size: 0.85rem; margin: 0 0 1.5rem; }
label { display: block; font-size: 0.8rem; color: #c8bfd5; margin: 0.9rem 0 0.35rem; }
input {
  width: 100%;
  padding: 0.6rem 0.75rem;
  background: #120e18;
  border: 1px solid #3a2f4a;
  border-radius: 6px;
  color: #e8e4dc;
  font-size: 0.95rem;
}
input:focus { outline: none; border-color: #b38b4a; }
button {
  margin-top: 1.25rem;
  width: 100%;
  padding: 0.7rem;
  background: #b38b4a;
  color: #120e18;
  border: 0;
  border-radius: 6px;
  font-weight: 600;
  font-size: 0.95rem;
  cursor: pointer;
}
button:hover { background: #c69d5b; }
.alt {
  text-align: center;
  margin-top: 1rem;
  font-size: 0.85rem;
  color: #9a93a8;
}
.alt a { color: #b38b4a; text-decoration: none; }
.alt a:hover { text-decoration: underline; }
.banner {
  padding: 0.6rem 0.75rem;
  border-radius: 6px;
  font-size: 0.85rem;
  margin-bottom: 1rem;
}
.banner.err { background: #3a1a1a; border: 1px solid #6e2e2e; color: #f2c6c6; }
.banner.info { background: #1a2a3a; border: 1px solid #2e4e6e; color: #c6d6f2; }
"#;

fn page(title: &str, body: &str) -> String {
    format!(
        r#"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8" />
<meta name="viewport" content="width=device-width, initial-scale=1" />
<title>{title} — Garlemald</title>
<style>{style}</style>
</head>
<body>
<div class="card">
{body}
</div>
</body>
</html>"#,
        title = escape(title),
        style = BASE_STYLE,
        body = body,
    )
}

fn banners(error: Option<&str>, info: Option<&str>) -> String {
    let mut s = String::new();
    if let Some(msg) = error.filter(|m| !m.is_empty()) {
        let _ = write!(s, r#"<div class="banner err">{}</div>"#, escape(msg));
    }
    if let Some(msg) = info.filter(|m| !m.is_empty()) {
        let _ = write!(s, r#"<div class="banner info">{}</div>"#, escape(msg));
    }
    s
}

pub fn login_page(error: Option<&str>, info: Option<&str>, prefill_username: &str) -> String {
    let body = format!(
        r#"
<h1>Sign in</h1>
<p class="subtitle">Garlemald / FINAL FANTASY XIV 1.23b</p>
{banners}
<form method="post" action="/login">
  <label for="username">Username</label>
  <input id="username" name="username" autocomplete="username"
         required maxlength="64" value="{user}" autofocus />
  <label for="password">Password</label>
  <input id="password" name="password" type="password"
         autocomplete="current-password" required maxlength="128" />
  <button type="submit">Sign in</button>
</form>
<p class="alt">No account yet? <a href="/signup">Create one</a></p>
"#,
        banners = banners(error, info),
        user = escape(prefill_username),
    );
    page("Sign in", &body)
}

pub fn signup_page(error: Option<&str>, prefill_username: &str, prefill_email: &str) -> String {
    let body = format!(
        r#"
<h1>Create account</h1>
<p class="subtitle">Garlemald / FINAL FANTASY XIV 1.23b</p>
{banners}
<form method="post" action="/signup">
  <label for="username">Username</label>
  <input id="username" name="username" autocomplete="username"
         required maxlength="64" pattern="[A-Za-z0-9_.-]{{3,64}}"
         value="{user}" autofocus
         title="3-64 characters: letters, numbers, underscore, dot, hyphen" />
  <label for="email">Email (optional)</label>
  <input id="email" name="email" type="email" autocomplete="email"
         maxlength="254" value="{email}" />
  <label for="password">Password</label>
  <input id="password" name="password" type="password"
         autocomplete="new-password" required minlength="8" maxlength="128" />
  <label for="confirm">Confirm password</label>
  <input id="confirm" name="confirm" type="password"
         autocomplete="new-password" required minlength="8" maxlength="128" />
  <button type="submit">Create account</button>
</form>
<p class="alt">Already have one? <a href="/login">Sign in</a></p>
"#,
        banners = banners(error, None),
        user = escape(prefill_username),
        email = escape(prefill_email),
    );
    page("Create account", &body)
}

/// Minimal HTML escaping for the fields we render back. We don't render
/// arbitrary user-controlled strings outside of form prefills and flash
/// messages, but better safe than sorry.
fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn login_page_escapes_prefill() {
        let html = login_page(None, None, "<script>x</script>");
        assert!(html.contains("&lt;script&gt;x&lt;/script&gt;"));
        assert!(!html.contains("<script>x</script>"));
    }

    #[test]
    fn login_page_renders_error_banner() {
        let html = login_page(Some("bad creds"), None, "");
        assert!(html.contains("bad creds"));
        assert!(html.contains("banner err"));
    }

    #[test]
    fn signup_page_renders_email_prefill() {
        let html = signup_page(None, "sam", "sam@example.com");
        assert!(html.contains("value=\"sam\""));
        assert!(html.contains("value=\"sam@example.com\""));
    }
}
