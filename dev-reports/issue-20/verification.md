# Issue #20 Verification

- Status: `passed`

## Checks

- ``ruby -e 'src=File.read("src/cli.rs"); flags=src.scan(/#\[arg\((.*?)\)\]\s*pub\s+(\w+):/m).select{|a,n| a.include?("long") && !a.include?("hide = true")}.map{|a,n| "--"+n.tr("_","-")}.sort; %w[en ja].each{|lang| docs=File.readlines("docs/guide/#{lang}/cli-reference.md").map{|l| l[/^\| `(--[^`]+)`/,1]}.compact.sort; abort("#{lang} CLI mismatch") unless flags==docs}; abort("expected 37 flags") unless flags.length==37'``: `passed`
- ``ruby -e 'block=File.read("src/tui/slash.rs").split("pub const SLASH_COMMANDS",2).last.split("pub fn slash_command_spec",2).first; names=block.scan(/name: "([^"]+)"/).flatten+block.scan(/aliases: &\[([^\]]*)\]/).flatten.flat_map{|s| s.scan(/"([^"]+)"/).flatten}; names.sort!; %w[en ja].each{|lang| docs=File.readlines("docs/guide/#{lang}/slash-commands.md").map{|l| l[/^\| `(\/[^`]+)`/,1]}.compact.sort; abort("#{lang} slash mismatch") unless names==docs}; abort("expected 15 names") unless names.length==15'``: `passed`
- `ruby -e 'stems=%w[cli-reference slash-commands configuration providers troubleshooting]; stems.each{|s| en=File.readlines("docs/guide/en/#{s}.md").map{|l| l[/\A(##+) /,1]}.compact; ja=File.readlines("docs/guide/ja/#{s}.md").map{|l| l[/\A(##+) /,1]}.compact; abort("heading mismatch: #{s}") unless en==ja}; files=(Dir["docs/guide/**/*.md"]+["docs/guide/README.md"]).uniq; bad=[]; files.each{|f| File.read(f).scan(/\[[^\]]+\]\(([^)]+)\)/).flatten.each{|link| next if link.start_with?("http://","https://","#"); target=File.expand_path(link.split("#",2).first,File.dirname(f)); bad << "#{f}: #{link}" unless File.exist?(target)}}; abort("missing links:\n"+bad.join("\n")) unless bad.empty?; abort("translation links") unless Dir["docs/guide/{en,ja}/*.md"].all?{|f| File.readlines(f).take(5).any?{|l| l.match?(/^\[(?:English|日本語)\]\(/)}}'`: `passed`
- `ruby -e 'pairs={"cli-reference"=>["num_predict","8192","max_iterations","12","chat_timeout_secs","600","180","chat_retries","1","context_budget","65536","--footer","--no-footer"],"slash-commands"=>["$(cat <path>)","--profile","--style","--prompt-layout","profile"],"configuration"=>[".anvil/config","ANVIL_NO_FOOTER","ANVIL_NO_SPINNER","ANVIL_NO_MARKDOWN","ANVIL_NO_INTERRUPT","prompt_layout","plan_preset"],"providers"=>["OPENAI_API_KEY","GEMINI_API_KEY","chmod 600 .env","--ollama-host"],"troubleshooting"=>["GEMINI_API_KEY is not set","port N is busy","interaction probe unavailable","--footer off","Model ID","Ollama"]}; pairs.each{|stem,needles| %w[en ja].each{|lang| text=File.read("docs/guide/#{lang}/#{stem}.md"); missing=needles.reject{|n| text.include?(n)}; abort("#{lang}/#{stem}: #{missing}") unless missing.empty?}}; index=File.read("docs/guide/README.md"); abort("model probe link") unless index.include?("../model-probe.md")'`: `passed`
- `cargo run --quiet -- --help`: `passed`
- `cargo test cli::tests`: `passed`
- `cargo test tui::slash::tests`: `passed`
- `cargo test config::tests`: `passed`
- `cargo test --lib preflight::tests`: `passed`
- `git diff --cached --check`: `passed`

## Scope note

This is a documentation-only change. No production Rust, schema, event,
fixture, runtime state, or release surface changed, so full `cargo test`,
clippy, and Rust formatting checks were not required.
