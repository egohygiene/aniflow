"""Authored/unrun fake service cases. No server, interpreter child or model."""
import copy
import unittest
from unittest.mock import patch
import provider


def request():
    return {'schema': 'aniflow.amv-ollama-invocation/v1',
            'packet': {'schema': 'aniflow.amv-evidence-packet/v1', 'media_samples': 0,
                       'full_movie_reviewed': False, 'packet_sha256': 'a' * 64},
            'identity': {'profile_sha256': 'b' * 64}, 'endpoint': '127.0.0.1:11434',
            'runtime_version': 'synthetic', 'model_name': 'synthetic:tag',
            'model_sha256': 'c' * 64, 'service_egress_disabled_declared': True,
            'context_tokens': 4096, 'output_tokens': 512, 'timeout_milliseconds': 1000,
            'template': 'Facts only, no commands.',
            'candidate_schema': {'properties': {k: {} for k in
                                 ['observed_fact_ids', 'suggestions', 'questions']}}}


def service(address, deadline, method, path, value=None):
    if path == '/api/version':
        return {'version': 'synthetic'}
    if path == '/api/tags':
        return {'models': [{'name': 'synthetic:tag', 'digest': 'c' * 64}]}
    return {'done': True, 'done_reason': 'stop', 'model': 'synthetic:tag',
            'message': {'role': 'assistant', 'content':
                        '{"observed_fact_ids":[],"suggestions":[],"questions":[]}'},
            'prompt_eval_count': 100, 'eval_count': 20}


class ProviderCases(unittest.TestCase):
    def test_one_call_and_current_identity_checked_before_after(self):
        with patch.object(provider, 'call', side_effect=service) as call:
            value = provider.invoke(request())
            self.assertEqual(value['model_calls'], 1)
            self.assertEqual([v.args[3] for v in call.call_args_list],
                             ['/api/version', '/api/tags', '/api/chat', '/api/version', '/api/tags'])
            chat = call.call_args_list[2].args[4]
            self.assertNotIn('tools', chat)
            self.assertFalse(chat['stream'])

    def test_remote_and_duplicate_json_refuse_without_service(self):
        value = request()
        value['endpoint'] = '192.0.2.1:11434'
        with patch.object(provider, 'call') as call:
            with self.assertRaises(provider.Refused):
                provider.invoke(value)
            call.assert_not_called()
        with self.assertRaises(provider.Refused):
            provider.decode('{"observed_fact_ids":[],"observed_fact_ids":[]}')

    def test_changed_model_refuses_after_reply(self):
        count = 0
        def changed(*args):
            nonlocal count
            value = service(*args)
            if args[3] == '/api/tags':
                count += 1
                if count == 2:
                    value['models'][0]['digest'] = 'd' * 64
            return value
        with patch.object(provider, 'call', side_effect=changed):
            with self.assertRaises(provider.Refused):
                provider.invoke(request())

    def test_tool_calls_truncation_and_usage_overrun_refuse(self):
        for change in ['tools', 'truncated', 'usage']:
            def malformed(*args):
                value = service(*args)
                if args[3] == '/api/chat':
                    if change == 'tools':
                        value['message']['tool_calls'] = [{'name': 'delete'}]
                    elif change == 'truncated':
                        value['done_reason'] = 'length'
                    else:
                        value['eval_count'] = 513
                return value
            with patch.object(provider, 'call', side_effect=malformed):
                with self.assertRaises(provider.Refused):
                    provider.invoke(copy.deepcopy(request()))
