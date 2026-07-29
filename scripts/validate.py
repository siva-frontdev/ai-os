#!/usr/bin/env python3
"""End-to-end validation harness for Desktop Operator prototype.

Feeds test inputs to desktop-agent REPL, captures tracing from stderr,
records pass/fail for each pipeline stage, produces CSV and summary.
"""

import subprocess
import sys
import os
import re
import json
import time
import shutil

RESULTS_DIR = "/home/arch/ai-os/target/validation"
os.makedirs(RESULTS_DIR, exist_ok=True)

# ── Test cases ────────────────────────────────────────────────
BROWSER_TESTS = [
    ("open firefox.com", "browser_open_url"),
    ("open google.com", "browser_open_url"),
    ("open github.com", "browser_open_url"),
    ("open youtube.com", "browser_open_url"),
    ("search for Rust async", "search_web"),
    ("search for AI Operating System", "search_web"),
    ("search for tokio tutorial", "search_web"),
]

APP_TESTS = [
    ("open firefox", "launch_app"),
    ("open chrome", "launch_app"),
    ("open VS Code", "launch_app"),
    ("open terminal", "launch_app"),
    ("open file manager", "launch_app"),
]

TYPING_TESTS = [
    ("type Hello World", "type_text"),
    ("type AI Operating System", "type_text"),
    ("type Rust", "type_text"),
]

KEYBOARD_TESTS = [
    ("press Enter", "keyboard"),
    ("press Escape", "keyboard"),
    ("press Ctrl+C", "keyboard"),
    ("press Ctrl+V", "keyboard"),
    ("press Ctrl+S", "keyboard"),
]

MOUSE_TESTS = [
    ("click", "mouse_click"),
    ("press", "mouse_click"),
]

WINDOW_TESTS = [
    ("focus Firefox", "focus_window"),
    ("focus VS Code", "focus_window"),
    ("focus Terminal", "focus_window"),
]

NEGATIVE_TESTS = [
    ("fly to the moon", "generic"),
    ("cook dinner", "generic"),
    ("teleport to Mars", "generic"),
    ("become invisible", "generic"),
]

STRESS_INPUTS = [
    "open firefox.com",
    "search for Rust async",
    "click",
    "press Escape",
    "type hello",
    "focus Firefox",
    "open google.com",
    "press Ctrl+C",
    "type world",
    "search for tokio tutorial",
] * 5  # 50 commands

ALL_TEST_CASES = (
    [("browser", *t) for t in BROWSER_TESTS] +
    [("app", *t) for t in APP_TESTS] +
    [("typing", *t) for t in TYPING_TESTS] +
    [("keyboard", *t) for t in KEYBOARD_TESTS] +
    [("mouse", *t) for t in MOUSE_TESTS] +
    [("window", *t) for t in WINDOW_TESTS] +
    [("negative", *t) for t in NEGATIVE_TESTS]
)


def run_test(proc, input_text: str, timeout: float = 10.0) -> tuple:
    """Send input to REPL, read output until next prompt appears."""
    import select

    # Drain any pending output
    time.sleep(0.15)

    proc.stdin.write(input_text + "\n")
    proc.stdin.flush()

    stdout_lines = []
    stderr_lines = []
    deadline = time.time() + timeout

    while time.time() < deadline:
        r, _, _ = select.select([proc.stdout, proc.stderr], [], [], 0.5)
        for fd in r:
            if fd == proc.stdout:
                line = proc.stdout.readline()
                if line:
                    stdout_lines.append(line.rstrip())
                    if "> " in line or "[ok]" in line or "[!!]" in line or "Error:" in line:
                        # Got a response, give a tiny bit more time for trailing output
                        time.sleep(0.1)
                        # Read any remaining stdout
                        while True:
                            r2, _, _ = select.select([proc.stdout], [], [], 0.05)
                            if not r2:
                                break
                            extra = proc.stdout.readline()
                            if extra:
                                stdout_lines.append(extra.rstrip())
                        return "\n".join(stdout_lines), "\n".join(stderr_lines)
            elif fd == proc.stderr:
                line = proc.stderr.readline()
                if line:
                    stderr_lines.append(line.rstrip())

    return "\n".join(stdout_lines), "\n".join(stderr_lines)


def parse_stages(stderr: str) -> dict:
    """Extract pipeline stages from tracing output."""
    stages = {
        "raw_input": "",
        "structured_json": {},
        "brain_goal": {},
        "tool_requirements": [],
        "execution_strategy": "",
        "osal_command": "",
        "desktop_observation_before": {},
        "desktop_observation_after": {},
        "verification": "",
        "result": "",
        "error": None,
    }

    lines = stderr.split("\n")
    for i, line in enumerate(lines):
        # Remove tracing timestamp/level prefix
        clean = re.sub(r'^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}.\d+Z\s+(INFO|WARN|ERROR)\s+', '', line)

        if "Raw User Input:" in clean:
            stages["raw_input"] = clean.split("Raw User Input:")[1].strip()
        elif "Structured JSON:" in clean:
            m = re.search(r'summary="([^"]*)"\s+intent="([^"]*)"', clean)
            if m:
                stages["structured_json"]["summary"] = m.group(1)
                stages["structured_json"]["intent"] = m.group(2)
            m2 = re.search(r'priority="?([A-Za-z]+)"?', clean)
            if m2:
                stages["structured_json"]["priority"] = m2.group(1)
        elif "entities:" in clean:
            entities_str = clean.split("entities:", 1)[1].strip()
            stages["structured_json"]["entities"] = entities_str
        elif "required_capabilities:" in clean:
            cap_str = clean.split("required_capabilities:", 1)[1].strip()
            stages["structured_json"]["required_capabilities"] = cap_str
        elif "BRAIN:" in clean and "goal_id" in clean:
            m = re.search(r'goal_id=([^\s]+)\s+plan_id=([^\s]+)\s+state=([^\s]+)', clean)
            if m:
                stages["brain_goal"] = {"goal_id": m.group(1), "plan_id": m.group(2), "state": m.group(3)}
        elif "Tool Requirements" in clean:
            m = re.search(r'\((\d+) total\)', clean)
            stages["tool_requirements_count"] = int(m.group(1)) if m else 0
        elif re.match(r'\s*\[(\d+)\]\s+capability="([^"]*)"', clean):
            m = re.search(r'\[(\d+)\]\s+capability="([^"]*)"\s+inputs=\[?(.*?)\]?$', clean)
            if m:
                stages["tool_requirements"].append({
                    "index": int(m.group(1)),
                    "capability": m.group(2),
                    "inputs": m.group(3),
                })
        elif "EXECUTION:" in clean:
            m = re.search(r'capability="([^"]*)"\s+params=\{(.*)\}', clean)
            if m:
                stages["execution_strategy"] = f"{m.group(1)}({m.group(2)})"
        elif "OPERATOR: observe BEFORE" in clean:
            m = re.search(r'(\d+) windows.*focus=([^,]+).*cursor=\(([^)]+)\).*clipboard=([^\s]+)', clean)
            if m:
                stages["desktop_observation_before"] = {
                    "windows": m.group(1), "focus": m.group(2),
                    "cursor": m.group(3), "clipboard": m.group(4),
                }
        elif "OPERATOR: observe AFTER" in clean:
            m = re.search(r'(\d+) windows.*focus=([^,]+).*cursor=\(([^)]+)\).*clipboard=([^\s]+)', clean)
            if m:
                stages["desktop_observation_after"] = {
                    "windows": m.group(1), "focus": m.group(2),
                    "cursor": m.group(3), "clipboard": m.group(4),
                }
        elif "OSAL result:" in clean:
            stages["osal_command"] = clean.split("OSAL result:")[1].strip()
        elif "OSAL error:" in clean:
            stages["osal_command"] = "ERROR: " + clean.split("OSAL error:")[1].strip()
        elif "verification:" in clean:
            stages["verification"] = clean.split("verification:")[1].strip()
        elif "TEMPLATE PROVIDER:" in clean and "response=" in clean:
            resp = clean.split("response=", 1)[1].strip() if "response=" in clean else ""
            try:
                stages["structured_json"]["raw_json"] = json.loads(resp)
            except json.JSONDecodeError:
                stages["structured_json"]["raw_json_raw"] = resp

    # Determine overall result
    stdout_parts = []
    return stages


def analyze_result(stderr: str, stdout: str) -> dict:
    """Analyze whether the pipeline succeeded and identify failures."""
    stages = parse_stages(stderr)

    result = {
        "stages": stages,
        "pass": True,
        "failures": [],
        "stdout": stdout,
        "stderr_summary": stderr[:500] if len(stderr) > 500 else stderr,
    }

    # Check template provider produced valid JSON
    if stages["structured_json"].get("raw_json"):
        result["template_ok"] = True
    elif stages["structured_json"].get("raw_json_raw"):
        result["template_ok"] = False
        result["failures"].append("template_provider")
        result["pass"] = False
    else:
        # Could be generic intent which is fine
        result["template_ok"] = True

    # Check brain produced a plan
    if stages["brain_goal"]:
        result["brain_ok"] = True
    else:
        result["brain_ok"] = False
        result["failures"].append("brain")
        result["pass"] = False

    # Check tool requirements
    if stages.get("tool_requirements_count", 0) > 0:
        result["requirements_ok"] = True
    else:
        result["requirements_ok"] = False
        result["failures"].append("brain_requirements")
        result["pass"] = False

    # Check execution happened
    if stages["execution_strategy"]:
        result["execution_ok"] = True
    else:
        result["execution_ok"] = False
        result["failures"].append("execution")
        result["pass"] = False

    # Check verification
    if "Success" in stages["verification"]:
        result["verification_ok"] = True
    elif "Failed" in stages["verification"] or "PartialSuccess" in stages["verification"]:
        result["verification_ok"] = False
        result["failures"].append("verification")
        result["pass"] = False
    else:
        result["verification_ok"] = None
        result["failures"].append("verification_unknown")

    # Check if any error was reported
    if "[!!]" in stdout:
        result["execution_error"] = True
        result["failures"].append("execution_error")
        if result["pass"]:  # Don't override if already failed
            result["pass"] = False

    return result


def main():
    print("Building desktop-agent...")
    build = subprocess.run(
        ["cargo", "build", "-p", "desktop-agent"],
        capture_output=True, text=True,
        cwd="/home/arch/ai-os"
    )
    if build.returncode != 0:
        print("BUILD FAILED:", build.stderr)
        sys.exit(1)

    binary = "/home/arch/ai-os/target/debug/desktop-agent"
    if not os.path.exists(binary):
        binary = shutil.which("desktop-agent")
    if not binary:
        print("Binary not found")
        sys.exit(1)

    print(f"Binary: {binary}")
    print(f"Running {len(ALL_TEST_CASES)} test cases...")
    print()

    all_results = []
    all_stderr = []

    for category, input_text, expected_intent in ALL_TEST_CASES:
        print(f"[{category}] {input_text} ... ", end="", flush=True)

        # Start fresh process for each test to avoid state contamination
        proc = subprocess.Popen(
            [binary],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            cwd="/home/arch/ai-os",
        )

        # Wait for prompt
        time.sleep(1.5)
        # Drain initial banner
        while True:
            import select
            r, _, _ = select.select([proc.stdout], [], [], 0.2)
            if not r:
                break
            proc.stdout.readline()

        stdout, stderr = run_test(proc, input_text)
        proc.terminate()
        proc.wait(timeout=5)

        result = analyze_result(stderr, stdout)
        result["category"] = category
        result["input"] = input_text
        result["expected_intent"] = expected_intent
        all_results.append(result)
        all_stderr.append(stderr)

        status = "PASS" if result["pass"] else "FAIL"
        failures = ",".join(result["failures"]) if result["failures"] else ""
        print(f"{status}" + (f" ({failures})" if failures else ""))

    # ── Print summary ──────────────────────────────────────────
    passed = sum(1 for r in all_results if r["pass"])
    failed = sum(1 for r in all_results if not r["pass"])
    total = len(all_results)

    print()
    print("=" * 60)
    print("  VALIDATION RESULTS")
    print("=" * 60)
    print(f"  Total:  {total}")
    print(f"  Passed: {passed}")
    print(f"  Failed: {failed}")
    print(f"  Rate:   {100 * passed / total:.1f}%")
    print()

    # Failures by component
    all_failures = []
    for r in all_results:
        all_failures.extend(r["failures"])
    from collections import Counter
    failure_counts = Counter(all_failures)
    if failure_counts:
        print("  Failures by component:")
        for comp, count in failure_counts.most_common():
            print(f"    {comp}: {count}")
    print()

    # ── Detailed failed cases ──────────────────────────────────
    print("  Failed cases:")
    for r in all_results:
        if not r["pass"]:
            fails = ", ".join(r["failures"])
            print(f"    [{r['category']}] \"{r['input']}\" → {fails}")
            print(f"      expected_intent={r['expected_intent']}")
            print(f"      template_ok={r.get('template_ok')} brain_ok={r.get('brain_ok')} "
                  f"req_ok={r.get('requirements_ok')} exec_ok={r.get('execution_ok')} "
                  f"ver_ok={r.get('verification_ok')}")
    print()

    # ── Template coverage ──────────────────────────────────────
    print("  Template patterns covered:")
    intents_seen = set()
    for r in all_results:
        intent = r["stages"]["structured_json"].get("intent", "")
        if intent:
            intents_seen.add(intent)
    for intent in sorted(intents_seen):
        matching = [r for r in all_results if r["stages"]["structured_json"].get("intent") == intent]
        p = sum(1 for r in matching if r["pass"])
        print(f"    {intent}: {p}/{len(matching)} passed")

    # ── Stress test ────────────────────────────────────────────
    print()
    print("=" * 60)
    print("  STRESS TEST (50 commands)")
    print("=" * 60)
    proc = subprocess.Popen(
        [binary],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        cwd="/home/arch/ai-os",
    )
    time.sleep(1.5)
    # Drain banner
    import select
    while True:
        r, _, _ = select.select([proc.stdout], [], [], 0.2)
        if not r:
            break
        proc.stdout.readline()

    stress_passed = 0
    stress_failed = 0
    stress_start = time.time()
    for i, cmd in enumerate(STRESS_INPUTS):
        stdout, stderr = run_test(proc, cmd, timeout=5)
        result = analyze_result(stderr, stdout)
        if result["pass"]:
            stress_passed += 1
        else:
            stress_failed += 1
    stress_elapsed = time.time() - stress_start

    proc.terminate()
    proc.wait(timeout=5)

    print(f"  Duration: {stress_elapsed:.1f}s")
    print(f"  Avg per command: {stress_elapsed/len(STRESS_INPUTS)*1000:.0f}ms")
    print(f"  Passed: {stress_passed}")
    print(f"  Failed: {stress_failed}")
    print(f"  Rate:   {100 * stress_passed / len(STRESS_INPUTS):.1f}%")

    # ── Write detailed report ──────────────────────────────────
    with open(os.path.join(RESULTS_DIR, "validation_results.json"), "w") as f:
        json.dump({
            "total": total,
            "passed": passed,
            "failed": failed,
            "rate_percent": round(100 * passed / total, 1),
            "failure_counts": dict(failure_counts),
            "results": [
                {
                    "input": r["input"],
                    "category": r["category"],
                    "pass": r["pass"],
                    "failures": r["failures"],
                    "intent": r["stages"]["structured_json"].get("intent", ""),
                    "template_ok": r.get("template_ok"),
                    "brain_ok": r.get("brain_ok"),
                    "requirements_ok": r.get("requirements_ok"),
                    "execution_ok": r.get("execution_ok"),
                    "verification_ok": r.get("verification_ok"),
                }
                for r in all_results
            ],
            "stress": {
                "passed": stress_passed,
                "failed": stress_failed,
                "total": len(STRESS_INPUTS),
                "duration_sec": round(stress_elapsed, 1),
            },
            "intents_covered": sorted(intents_seen),
        }, f, indent=2)

    print(f"\n  Detailed results: {RESULTS_DIR}/validation_results.json")
    print()


if __name__ == "__main__":
    main()
