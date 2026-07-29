#!/bin/bash
# Direct validation: feeds each input to desktop-agent, captures results.
# Avoids fragile Python parsing by using grep on stderr markers.

BINARY="/home/arch/ai-os/target/debug/desktop-agent"
RESULTS="/home/arch/ai-os/target/validation"
mkdir -p "$RESULTS"
rm -f "$RESULTS"/*.txt

fail_if_stderr_empty() {
    local stderr_file="$1"
    local label="$2"
    if [ ! -s "$stderr_file" ]; then
        echo "  WARN: $label stderr is empty (process may have crashed)"
    fi
}

run_one() {
    local category="$1"
    local input="$2"
    local safe="${3:-$input}"
    local outfile="$RESULTS/out_${category}_${safe// /_}.txt"
    local errfile="$RESULTS/err_${category}_${safe// /_}.txt"

    echo "[$category] $input" | tee -a "$RESULTS/summary.txt"

    # Run and capture output with timeout
    timeout 6 "$BINARY" <<< "$input" > "$outfile" 2> "$errfile" || true

    fail_if_stderr_empty "$errfile" "$category"

    # Extract pipeline stages
    local template_json=$(grep -oP '\[TEMPLATE\] response \(\d+ bytes\): \K.*' "$errfile" | head -1)
    local template_intent=$(echo "$template_json" | python3 -c "import sys,json; d=json.loads(sys.stdin.read()); print(d.get('intent',''))" 2>/dev/null || echo "parse_fail")
    local template_conf=$(echo "$template_json" | python3 -c "import sys,json; d=json.loads(sys.stdin.read()); print(d.get('confidence',''))" 2>/dev/null || echo "?")

    # Check execution result
    local exec_ok="no"
    if grep -q '\[ok\]' "$outfile"; then exec_ok="yes"; fi
    local exec_fail="no"
    if grep -q '\[!!\]' "$outfile"; then exec_fail="yes"; fi

    # Determine which stage failed
    local failure=""
    if [ -z "$template_intent" ] || [ "$template_intent" = "parse_fail" ]; then
        failure="TEMPLATE"
    elif grep -q "NoModelFound\|routing failed" "$errfile"; then
        failure="ROUTING"
    elif [ "$exec_fail" = "yes" ]; then
        if grep -q "No such file or directory\|command not found" "$errfile" "$outfile"; then
            failure="OSAL_MISSING_TOOL"
        else
            failure="EXECUTION"
        fi
    fi
    if [ -z "$failure" ] && [ "$exec_ok" != "yes" ] && [ "$exec_fail" != "yes" ]; then
        failure="UNKNOWN"
    fi

    # Summary line
    local status="PASS"
    [ -n "$failure" ] && status="FAIL:$failure"
    echo "  → intent=$template_intent confidence=$template_conf exec_ok=$exec_ok status=$status"
    echo "$category | $input | $template_intent | $template_conf | $exec_ok | $status" >> "$RESULTS/results.csv"
}

echo "CATEGORY | INPUT | INTENT | CONFIDENCE | EXEC_OK | STATUS" > "$RESULTS/results.csv"
echo "========================================" > "$RESULTS/summary.txt"

# ── Browser tests ──
echo "────────── Browser Tests ──────────" >> "$RESULTS/summary.txt"
run_one "browser" "open firefox.com" "open_firefox_com"
run_one "browser" "open google.com" "open_google_com"
run_one "browser" "open github.com" "open_github_com"
run_one "browser" "open youtube.com" "open_youtube_com"
run_one "browser" "search for Rust async" "search_rust_async"
run_one "browser" "search for AI Operating System" "search_ai_os"
run_one "browser" "search for tokio tutorial" "search_tokio"

# ── App tests ──
echo "" >> "$RESULTS/summary.txt"
echo "────────── Application Tests ──────────" >> "$RESULTS/summary.txt"
run_one "app" "open firefox" "open_firefox"
run_one "app" "open chrome" "open_chrome"
run_one "app" "open VS Code" "open_vscode"
run_one "app" "open terminal" "open_terminal"
run_one "app" "open file manager" "open_file_manager"

# ── Typing tests ──
echo "" >> "$RESULTS/summary.txt"
echo "────────── Typing Tests ──────────" >> "$RESULTS/summary.txt"
run_one "typing" "type Hello World" "type_hello"
run_one "typing" "type AI Operating System" "type_ai_os"
run_one "typing" "type Rust" "type_rust"

# ── Keyboard tests ──
echo "" >> "$RESULTS/summary.txt"
echo "────────── Keyboard Tests ──────────" >> "$RESULTS/summary.txt"
run_one "keyboard" "press Enter" "press_enter"
run_one "keyboard" "press Escape" "press_escape"
run_one "keyboard" "press Ctrl+C" "press_ctrlc"
run_one "keyboard" "press Ctrl+V" "press_ctrlv"
run_one "keyboard" "press Ctrl+S" "press_ctrls"

# ── Mouse tests ──
echo "" >> "$RESULTS/summary.txt"
echo "────────── Mouse Tests ──────────" >> "$RESULTS/summary.txt"
run_one "mouse" "click" "click"
run_one "mouse" "press" "press"

# ── Window tests ──
echo "" >> "$RESULTS/summary.txt"
echo "────────── Window Tests ──────────" >> "$RESULTS/summary.txt"
run_one "window" "focus Firefox" "focus_firefox"
run_one "window" "focus VS Code" "focus_vscode"
run_one "window" "focus Terminal" "focus_terminal"

# ── Negative tests ──
echo "" >> "$RESULTS/summary.txt"
echo "────────── Negative Tests ──────────" >> "$RESULTS/summary.txt"
run_one "negative" "fly to the moon" "fly_moon"
run_one "negative" "cook dinner" "cook_dinner"
run_one "negative" "teleport to Mars" "teleport_mars"
run_one "negative" "become invisible" "become_invisible"

# ── Summary stats ──
echo ""
echo "========================================"
echo "  VALIDATION SUMMARY"
echo "========================================"
python3 -c "
import csv, collections
with open('$RESULTS/results.csv') as f:
    reader = csv.DictReader(f, delimiter='|')
    rows = list(reader)
total = len(rows)
pass_count = sum(1 for r in rows if r['STATUS'].strip() == 'PASS')
fail_count = total - pass_count
print(f'  Total: {total}')
print(f'  Passed: {pass_count}')
print(f'  Failed: {fail_count}')
print(f'  Rate: {100*pass_count/total:.1f}%')
print()

# Failures by component
failures = collections.Counter()
for r in rows:
    s = r['STATUS'].strip()
    if s.startswith('FAIL:'):
        failures[s.split(':',1)[1]] += 1
if failures:
    print('  Failures by component:')
    for comp, cnt in failures.most_common():
        print(f'    {comp}: {cnt}')
print()

# Intents covered
intents = set()
for r in rows:
    i = r['INTENT'].strip()
    if i:
        intents.add(i)
print('  Intents covered:')
for i in sorted(intents):
    cnt = sum(1 for r in rows if r['INTENT'].strip() == i)
    p = sum(1 for r in rows if r['INTENT'].strip() == i and r['STATUS'].strip() == 'PASS')
    print(f'    {i}: {cnt} tests ({p} pass)')
"
