//! Symbols named by a changed line: the things a commit added or removed
//! that a later question is likely to be about. Regex-level on purpose:
//! this runs over every added and removed line of tens of thousands of
//! commits, and a def or a setting key is unambiguous at line level.

use regex::Regex;
use std::sync::OnceLock;

/// (kind, name)
pub type Symbol = (&'static str, String);

struct Patterns {
    rb_def: Regex,
    rb_class: Regex,
    setting_use: Regex,
    route: Regex,
    schema: Regex,
    plugin_api_rb: Regex,
    setting_key: Regex,
    js_class: Regex,
    js_api: Regex,
    js_fn: Regex,
}

fn patterns() -> &'static Patterns {
    static P: OnceLock<Patterns> = OnceLock::new();
    P.get_or_init(|| Patterns {
        rb_def: Regex::new(r"^\s*def\s+(self\.)?([A-Za-z_]\w*[?!=]?)").unwrap(),
        rb_class: Regex::new(r"^\s*(class|module)\s+([A-Z][\w:]*)").unwrap(),
        setting_use: Regex::new(r"SiteSetting\.([a-z][a-z0-9_]*)").unwrap(),
        route: Regex::new(
            r#"^\s*(get|post|put|patch|delete|match|resources|resource)\s+["':]([^"',\s]+)"#,
        )
        .unwrap(),
        schema: Regex::new(
            r"\b(create_table|drop_table|add_column|remove_column|rename_column|add_index|remove_index|add_reference)\s+:(\w+)",
        )
        .unwrap(),
        plugin_api_rb: Regex::new(
            r"\b(register_[a-z_]+|add_to_serializer|add_to_class|add_model_callback|add_admin_route|add_api_key_scope|on)\b\s*[( :]",
        )
        .unwrap(),
        setting_key: Regex::new(r"^  ([a-z][a-z0-9_]*):").unwrap(),
        js_class: Regex::new(r"export\s+default\s+class\s+(\w+)").unwrap(),
        js_api: Regex::new(r"\bapi\.([a-zA-Z]\w*)\(").unwrap(),
        js_fn: Regex::new(r"^\s*(?:async\s+)?([a-z]\w*)\s*\([^()]*\)\s*\{\s*$").unwrap(),
    })
}

const JS_KEYWORDS: &[&str] = &[
    "if",
    "for",
    "while",
    "switch",
    "catch",
    "function",
    "return",
    "constructor",
];

/// Files whose lines carry no symbols worth keeping.
pub fn skip_path(path: &str) -> bool {
    path.ends_with(".lock")
        || path.ends_with(".svg")
        || path.ends_with(".min.js")
        || path.contains("/locales/")
        || path.contains("/fixtures/")
        || path.starts_with("vendor/")
}

fn is_settings_yml(path: &str) -> bool {
    path.ends_with("config/site_settings.yml") || path.ends_with("config/settings.yml")
}

/// Symbols named on one added or removed line of `path`.
pub fn extract(path: &str, line: &str) -> Vec<Symbol> {
    let p = patterns();
    let mut out = Vec::new();

    if is_settings_yml(path) {
        if let Some(c) = p.setting_key.captures(line) {
            out.push(("setting", c[1].to_string()));
        }
        return out;
    }

    let ruby = path.ends_with(".rb") || path.ends_with(".rake");
    let js = path.ends_with(".js") || path.ends_with(".gjs") || path.ends_with(".ts");

    if ruby {
        if let Some(c) = p.rb_def.captures(line) {
            let name = match c.get(1) {
                Some(_) => format!("self.{}", &c[2]),
                None => c[2].to_string(),
            };
            out.push(("def", name));
        }
        if let Some(c) = p.rb_class.captures(line) {
            out.push((
                if &c[1] == "class" { "class" } else { "module" },
                c[2].to_string(),
            ));
        }
        if path.ends_with("routes.rb") {
            if let Some(c) = p.route.captures(line) {
                out.push(("route", format!("{} {}", &c[1], &c[2])));
            }
        }
        if path.contains("db/migrate/") || path.contains("db/post_migrate/") {
            for c in p.schema.captures_iter(line) {
                out.push(("schema", format!("{} {}", &c[1], &c[2])));
            }
        }
        if path.ends_with("plugin.rb") {
            for c in p.plugin_api_rb.captures_iter(line) {
                out.push(("plugin_api", c[1].to_string()));
            }
        }
    }

    if js {
        if let Some(c) = p.js_class.captures(line) {
            out.push(("js_class", c[1].to_string()));
        }
        for c in p.js_api.captures_iter(line) {
            out.push(("plugin_api", format!("api.{}", &c[1])));
        }
        if let Some(c) = p.js_fn.captures(line) {
            if !JS_KEYWORDS.contains(&&c[1]) {
                out.push(("js_fn", c[1].to_string()));
            }
        }
    }

    if ruby || js {
        for c in p.setting_use.captures_iter(line) {
            out.push(("setting_use", c[1].to_string()));
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ruby_symbols() {
        assert_eq!(
            extract("app/models/post.rb", "  def self.cook(raw)"),
            vec![("def", "self.cook".into())]
        );
        assert_eq!(
            extract("app/models/post.rb", "class Post < ActiveRecord::Base"),
            vec![("class", "Post".into())]
        );
        assert_eq!(
            extract(
                "app/controllers/x_controller.rb",
                "    raise Discourse::NotFound unless SiteSetting.taper_enabled"
            ),
            vec![("setting_use", "taper_enabled".into())]
        );
    }

    #[test]
    fn settings_routes_migrations() {
        assert_eq!(
            extract("config/site_settings.yml", "  taper_enabled:"),
            vec![("setting", "taper_enabled".into())]
        );
        assert!(extract("config/site_settings.yml", "taper:").is_empty());
        assert_eq!(
            extract("config/routes.rb", "  get \"/taper\" => \"taper#index\""),
            vec![("route", "get /taper".into())]
        );
        assert_eq!(
            extract(
                "db/migrate/2026_x.rb",
                "    add_column :posts, :cooked, :text"
            ),
            vec![("schema", "add_column posts".into())]
        );
        assert_eq!(
            extract("plugin.rb", "  register_homepage(\"taper\")"),
            vec![("plugin_api", "register_homepage".into())]
        );
    }

    #[test]
    fn js_symbols() {
        assert_eq!(
            extract(
                "app/assets/javascripts/discourse/app/components/x.gjs",
                "export default class TaperRow extends Component {"
            ),
            vec![("js_class", "TaperRow".into())]
        );
        assert_eq!(
            extract("x.js", "    api.decorateCookedElement(fn);"),
            vec![("plugin_api", "api.decorateCookedElement".into())]
        );
        assert_eq!(
            extract("x.js", "  async refresh() {"),
            vec![("js_fn", "refresh".into())]
        );
        assert!(extract("x.js", "  if (x) {").is_empty());
    }

    #[test]
    fn skipped_paths() {
        assert!(skip_path("config/locales/server.en.yml"));
        assert!(skip_path("pnpm-lock.yaml.lock"));
        assert!(!skip_path("app/models/post.rb"));
    }
}
