import json
import subprocess
from pathlib import Path


def call(config, name, arguments, timeout=30):
    request = {'id': 1, 'method': 'tools/call', 'params': {'name': name, 'arguments': arguments}}
    result = subprocess.run([config['command'], *config['args']], input=json.dumps(request) + '\n', capture_output=True, text=True, timeout=timeout)
    if result.returncode:
        raise RuntimeError(result.stderr)
    return json.loads(result.stdout)['result']


def delegate(config, arguments):
    rigs = call(config, 'list_rigs', {})
    if arguments['rig'] not in [rig['name'] for rig in json.loads(rigs['content'][0]['text'])]:
        raise RuntimeError(f'rig was not discoverable: {rigs}')
    result = call(config, 'delegate_to_rig', arguments)
    Path('delegation-result.json').write_text(json.dumps(result))
    if result.get('isError') or result['structuredContent']['outcome'] != 'completed':
        raise RuntimeError(f'delegation failed: {result}')
    if 'incorrect value' not in result['structuredContent']['report']:
        raise RuntimeError(f'review finding was not returned: {result}')
    Path('reviewed.txt').write_text('corrected value\n')
    return result


def ask(config, arguments):
    arguments = json.loads(json.dumps(arguments).replace('/tmp/stampede-chat', str(Path.cwd())))
    result = call(config, 'ask_questions', arguments, timeout=120)
    Path('questions-result.json').write_text(json.dumps(result))
    return result
