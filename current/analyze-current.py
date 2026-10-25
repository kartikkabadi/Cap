from pathlib import Path
import collections, csv, hashlib, json, math, re, statistics, sys

p = Path(__file__).resolve().parent

def diagnostics(path):
    rows = [json.loads(s) for s in path.read_text().splitlines()]
    return [r['message'] for r in rows if r.get('reason') == 'compiler-message'
            and 'cap-desktop-gpui' in r.get('package_id', '')
            and r['message']['level'] in ['warning', 'error']]

def key(message):
    return (message.get('code', {}).get('code') if message.get('code') else None, message['message'])

baseline = json.loads((p/'baseline/geometry.json').read_text())
candidate = json.loads((p/'candidate/geometry.json').read_text())
assert baseline == candidate and len(candidate) == 480
native_b = json.loads((p/'baseline/native/manifest.json').read_text())
native_c = json.loads((p/'candidate/native/manifest.json').read_text())
assert native_b == native_c and len(native_c) == 108
pixels = []
for case in native_c:
    filename = f"{case['id']:03}.png"
    before = (p/'baseline/native'/filename).read_bytes()
    after = (p/'candidate/native'/filename).read_bytes()
    assert before == after, filename
    pixels.append({'file': filename, 'sha256': hashlib.sha256(after).hexdigest()})
before = diagnostics(p/'baseline/clippy.jsonl')
after = diagnostics(p/'committed/clippy.jsonl')
assert collections.Counter(map(key, before)) == collections.Counter(map(key, after))
assert not [m for m in after if any(s['file_name'].endswith('ui/text_input.rs') for s in m['spans'])]
assert not [m for m in after if m['level'] == 'error']
profiles = [json.loads((p/name/'profile.json').read_text())['profile'] for name in ['baseline','candidate','committed']]
assert profiles[0] == profiles[1]
assert profiles[0] == {'opt_level':'3','debuginfo':0,'debug_assertions':True,'overflow_checks':True,'test':True}
assert profiles[2] == {'opt_level':'0','debuginfo':0,'debug_assertions':True,'overflow_checks':True,'test':True}
def build_configs(name):
    configs = collections.Counter()
    for line in (p/name/'build.jsonl').read_text().splitlines():
        r = json.loads(line)
        if r.get('reason') == 'compiler-artifact':
            configs[(r['package_id'],r['target']['name'],tuple(r['target']['kind']),tuple(r['features']),json.dumps(r['profile'],sort_keys=True))] += 1
    return configs
before_configs = build_configs('baseline')
assert before_configs == build_configs('candidate')
(p/'benchmark-build-comparison.json').write_text(json.dumps({'compiler_artifact_targets':sum(before_configs.values()),'all_dependency_and_app_features_and_profiles_match':True,'comparison':'multiset preserves distinct build/target variants of a package'},indent=2)+'\n')
tests = {}
for name in ['baseline','candidate','committed']:
    matches = re.findall(r'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored;', (p/name/'tests.log').read_text())
    assert len(matches) == 1, (name, matches)
    passed, failed, ignored = map(int, matches[0])
    assert failed == 0
    tests[name] = {'passed':passed,'failed':failed,'ignored':ignored}
assert (p/'baseline/cache-status.txt').read_text().strip() == '101'
assert 'warm layout replaced during: ["right", "shift-right", "scroll"]' in (p/'baseline/cache-diagnostic.log').read_text()
assert (p/'candidate/cache-status.txt').read_text().strip() == '0'
result = {'benchmark_base':'ec15b98403441aee7780ff7734fef93d2e1a323f', 'committed_test_base':json.loads((p/'committed/source.json').read_text())['base'], 'checked_main':json.loads((p/'latest-main-equivalence.json').read_text())['latest_checked_main'], 'tests':tests,
          'geometry_cases':480, 'native_cases':108, 'native_images':pixels,
          'native_image_sizes':sorted({tuple(x['image_size']) for x in native_c}),
          'clippy':{'baseline_diagnostics':len(before),'candidate_diagnostics':len(after),'new_diagnostics':0,'changed_file_diagnostics':0},
          'benchmark_profile':profiles[0], 'committed_suite_profile':profiles[2], 'cache_diagnostic':{'baseline':'fails for right, shift-right and scroll','candidate':'passes'}}
(p/'validation.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps({k:v for k,v in result.items() if k!='native_images'},indent=2))
if '--validation-only' in sys.argv:
    raise SystemExit(0)

folder = p/'performance-results'
rows = list(csv.DictReader((folder/'summary.csv').open()))
assert len(rows) == 66
keys = list(rows[0])[:list(rows[0]).index('baseline_median_ns')]
samples = {}
for pair in range(1,4):
    for variant in ['baseline','candidate']:
        grouped = collections.defaultdict(list)
        for r in csv.DictReader((folder/f'pair-{pair}-{variant}.csv').open()):
            k = tuple(r[f] for f in keys)
            ns = float(r['ns_per_step'])
            assert ns > 0 and math.isfinite(ns)
            grouped[k].append((int(r['sample']), ns))
        assert len(grouped) == 66
        for k, values in grouped.items():
            assert sorted(i for i,_ in values) == list(range(1,8))
            samples.setdefault(k,{})[pair,variant] = statistics.median(n for _,n in values)
for r in rows:
    s = samples[tuple(r[f] for f in keys)]
    before = [s[i,'baseline'] for i in range(1,4)]
    after = [s[i,'candidate'] for i in range(1,4)]
    ratios = [a/b for a,b in zip(before,after)]
    for col,value in [('baseline_median_ns',statistics.median(before)),('candidate_median_ns',statistics.median(after)),('median_paired_speedup',statistics.median(ratios)),('paired_min',min(ratios)),('paired_max',max(ratios))]:
        assert math.isclose(float(r[col]),value,rel_tol=1e-12), (col,r)
summary = {'raw_samples':2772,'cases':66,'pairs':3,'ranges':{},'ascii_middle':[]}
for target in ['10000','100000']:
    for scope in ['key_dispatch','scroll_redraw']:
        selected = [r for r in rows if r['target_bytes']==target and r['scope']==scope]
        ratios = [float(r['median_paired_speedup']) for r in selected]
        summary['ranges'][target+'_'+scope] = {'min':min(ratios),'max':max(ratios),'cases':len(selected)}
        for r in selected:
            if r['kind']=='ascii' and r['region']=='middle':
                summary['ascii_middle'].append({k:r[k] for k in ['target_bytes','motion','scope','baseline_median_ns','candidate_median_ns','median_paired_speedup','paired_min','paired_max']})
(p/'performance-analysis.json').write_text(json.dumps(summary,indent=2)+'\n')
print(json.dumps(summary,indent=2))
