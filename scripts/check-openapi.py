#!/usr/bin/env python3
"""Validate OpenAPI and, optionally, saved private read responses."""
import argparse
import copy
import json
from pathlib import Path
import yaml
from jsonschema import Draft4Validator
from openapi_spec_validator import validate

ROOT = Path(__file__).resolve().parents[1]

def json_schema(value):
    """Translate OpenAPI 3.0 nullable to JSON Schema Draft 4."""
    if isinstance(value, list):
        return [json_schema(v) for v in value]
    if not isinstance(value, dict):
        return value
    result = {k: json_schema(v) for k, v in value.items() if k != 'nullable'}
    if value.get('nullable'):
        return {'anyOf': [result, {'type': 'null'}]}
    return result

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--responses', type=Path, help='Private .local/api directory; no values are printed')
args = parser.parse_args()
spec = yaml.safe_load((ROOT / 'spec/openapi.yaml').read_text())
validate(spec)
names = set()
checked = 0
for path, item in spec['paths'].items():
    for method, operation in item.items():
        for alias in operation.get('x-cli-operations', []):
            assert alias['name'] not in names, 'Duplicate CLI operation'
            names.add(alias['name'])
            request = operation.get('requestBody', {}).get('content', {}).get('application/json', {}).get('schema')
            if alias.get('request_schema'):
                assert alias['request_schema'] in spec['components']['schemas'], 'Unknown alias request schema'
            if 'generated_body' in alias:
                assert request is not None, 'Generated body requires a request schema'
                generated_schema = copy.deepcopy(request)
                generated_schema['components'] = spec['components']
                Draft4Validator(json_schema(generated_schema)).validate(alias['generated_body'])
            if args.responses is None or alias.get('projection') or alias.get('lookup'):
                continue
            response = args.responses / (alias['name'] + '-response.json')
            if not response.exists():
                continue
            saved = json.loads(response.read_text())
            schema = copy.deepcopy(operation['responses']['200']['content']['application/json']['schema'])
            schema['components'] = spec['components']
            errors = list(Draft4Validator(json_schema(schema)).iter_errors(saved['data']))
            if errors:
                # Errors may contain private values; only report paths, never their messages.
                raise SystemExit(f"{alias['name']}: response schema mismatch at " + str(list(errors[0].absolute_path)))
            checked += 1
print(f'OpenAPI valid; {len(names)} CLI operations; {checked} saved responses validated')
