"""Browser interaction checks for the public-SDK-only, explicitly synthetic showroom."""
import contextlib
import json
import re
import os
import pathlib
import subprocess
import sys
import tempfile
import time
import unittest
from playwright.sync_api import sync_playwright, expect
ROOT = pathlib.Path(__file__).resolve().parents[1]


class DemoTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.temp = tempfile.TemporaryDirectory(prefix='originlm-browser-test-')
        bundle = pathlib.Path(cls.temp.name) / 'fixture-bundle.js'
        result = subprocess.run(['node', 'scripts/bundle_demo_test.mjs', str(bundle)], cwd=ROOT, capture_output=True, text=True)
        if result.returncode:
            raise RuntimeError(result.stderr)
        cls.bundle = bundle.read_text(encoding="utf-8")
        # Styles are the real Tailwind output from the Vite build (`npm run build:demo`).
        sheets = sorted((ROOT / 'apps/demo/dist/assets').glob('*.css'))
        if not sheets:
            raise RuntimeError('Missing demo stylesheet; run `npm run build:demo` first')
        cls.styles = '\n'.join(sheet.read_text(encoding="utf-8") for sheet in sheets)
        cls.playwright = sync_playwright().start()
        executable = os.environ.get('CHROMIUM_EXECUTABLE')
        cls.browser = cls.playwright.chromium.launch(executable_path=executable, headless=True, args=['--no-sandbox'])

    @classmethod
    def tearDownClass(cls):
        cls.browser.close()
        cls.playwright.stop()
        cls.temp.cleanup()

    def setUp(self):
        self.page = self.browser.new_page(viewport={'width':1440,'height':1000}, reduced_motion='reduce')
        self.errors = []
        self.page.on('pageerror', lambda error: self.errors.append(str(error)))

    def tearDown(self):
        self.page.close()
        self.assertEqual(self.errors, [])

    def open_playground(self):
        # Nothing is selected until a person picks a route, so choose the first one from the composer.
        self.page.locator('[data-slot="sidebar"]').get_by_role('link',name='Playground',exact=True).click()
        self.page.get_by_role('button',name='Choose a model and account').click()
        self.page.get_by_role('dialog').get_by_role('option').first.click()
        expect(self.page.get_by_role('button',name=re.compile(r'^Model:'))).to_be_visible()

    def load(self, fixture_options=None):
        # Offline browser rendering respects the container's navigation restriction.
        # Network/HTTP semantics are tested independently by the real HTTP SDK tests.
        self.page.set_content('<!doctype html><html lang="en"><head><meta charset="UTF-8"><title>OriginLM — Developer Showcase</title></head><body><div id="app"></div></body></html>')
        self.page.add_style_tag(content=self.styles)
        self.page.add_script_tag(content=self.bundle)
        self.page.evaluate('(options) => window.mountTestApp(options)', fixture_options or {})
        expect(self.page.locator('#mode-banner')).to_contain_text('Synthetic')
        expect(self.page.locator('#page-title')).to_have_text('Selector lab')

    def test_provider_rail_and_native_options(self):
        self.load()
        expect(self.page.get_by_test_id('provider-rail')).to_be_visible()
        self.page.get_by_test_id('model-codex-personal-0').click()
        effort=self.page.get_by_role('combobox', name='Reasoning effort', exact=True)
        effort.click()
        self.page.get_by_role('option', name='Cosmic', exact=True).click()
        expect(self.page.locator('#selection-json')).to_contain_text('cosmic')
        self.page.get_by_test_id('model-codex-personal-1').click()
        expect(self.page.get_by_role('combobox', name='Reasoning effort', exact=True)).to_have_count(0)

    def test_all_eight_variants_preserve_the_same_selection(self):
        self.load()
        self.page.get_by_test_id('model-codex-personal-0').click()
        before=self.page.locator('#selection-json').text_content()
        for variant in ['compact','palette','capability','compare','allowance','preset','mobile','rail']:
            self.page.locator(f'[data-variant="{variant}"]').click()
            expect(self.page.locator('#variant-view')).to_have_attribute('data-current-variant',variant)
            self.assertEqual(self.page.locator('#selection-json').text_content(),before)

    def test_favorites_retain_route_identity(self):
        self.load()
        self.page.get_by_test_id('model-codex-work-0').click()
        self.page.get_by_test_id('favorite-current').click()
        self.page.locator('[data-provider="favorites"]').click()
        expect(self.page.get_by_test_id('model-codex-work-0')).to_be_visible()
        expect(self.page.get_by_test_id('model-codex-personal-0')).to_have_count(0)

    def test_capability_requirements_do_not_treat_unknown_as_supported(self):
        self.load()
        self.page.locator('[data-variant="capability"]').click()
        self.page.get_by_role('checkbox',name='Structured output',exact=True).check()
        self.page.get_by_role('checkbox',name='Only eligible routes',exact=True).check()
        expect(self.page.get_by_test_id('model-codex-personal-0')).to_have_count(0)
        expect(self.page.get_by_test_id('model-codex-personal-1')).to_be_visible()

    def test_usage_unknown_total_has_no_percentage_gauge(self):
        self.load()
        self.page.locator('[data-slot="sidebar"]').get_by_role('link',name='Usage',exact=True).click()
        card=self.page.locator('[data-pool="pool-unknown-total"]')
        expect(card).to_contain_text('17')
        expect(card.locator('progress')).to_have_count(0)
        expect(self.page.locator('[data-pool="pool-shared-personal"]')).to_have_count(1)

    def test_playground_streams_through_sdk(self):
        self.load()
        self.open_playground()
        self.page.get_by_role('checkbox',name='Allow this simulated session',exact=True).check()
        self.page.get_by_role('button',name='Simulate turn',exact=True).click()
        expect(self.page.locator('#response-text')).to_contain_text('No AI provider was called',timeout=5000)
        expect(self.page.locator('#session-status')).to_contain_text('Completed')

    def test_failed_turn_explains_safe_error_code_without_showing_provider_text(self):
        self.load({'turnError': {'code': 'PROVIDER_UNAVAILABLE', 'message': 'PRIVATE_PROVIDER_DETAIL'}})
        self.open_playground()
        self.page.get_by_role('checkbox',name='Allow this simulated session',exact=True).check()
        self.page.get_by_role('button',name='Simulate turn',exact=True).click()
        expect(self.page.get_by_role('alert')).to_contain_text('provider client stopped or returned an unavailable response',timeout=5000)
        expect(self.page.get_by_role('alert')).to_contain_text('PROVIDER_UNAVAILABLE')
        expect(self.page.get_by_test_id('turn-debug')).to_contain_text('PRIVATE_PROVIDER_DETAIL')

    def test_playground_picker_selects_provider_and_route_without_selector_lab(self):
        self.load()
        self.page.locator('[data-slot="sidebar"]').get_by_role('link',name='Playground',exact=True).click()
        self.page.get_by_role('button',name='Choose a model and account').click()
        picker=self.page.get_by_role('dialog')
        # Real provider marks label the rail; the list shows every route for the chosen provider.
        picker.get_by_role('button',name='OpenCode',exact=True).click()
        options=picker.get_by_role('option')
        expect(options).to_have_count(4)
        texts=options.all_text_contents()
        self.assertTrue(any('Linked account' in text for text in texts))
        self.assertTrue(any('API-key route' in text for text in texts))
        options.filter(has_text='Example Balanced').filter(has_text='Linked account').first.click()
        expect(self.page.locator('#configuration-panel')).to_contain_text('Example Balanced')
        expect(self.page.locator('#configuration-panel')).to_contain_text('Linked account')

    def test_tool_approval_can_be_denied(self):
        self.load()
        self.open_playground()
        self.page.get_by_role('checkbox',name='Allow this simulated session',exact=True).check()
        self.page.get_by_role('checkbox',name='Simulate a tool approval',exact=True).check()
        self.page.get_by_role('button',name='Simulate turn',exact=True).click()
        self.page.get_by_role('button',name='Deny',exact=True).click()
        expect(self.page.locator('#session-status')).to_contain_text('Denied')

    def test_cancel_stops_a_simulated_turn(self):
        self.load()
        self.open_playground()
        self.page.get_by_role('checkbox',name='Allow this simulated session',exact=True).check()
        self.page.get_by_role('checkbox',name='Simulate a tool approval',exact=True).check()
        self.page.get_by_role('button',name='Simulate turn',exact=True).click()
        self.page.get_by_role('button',name='Cancel turn',exact=True).click()
        expect(self.page.locator('#session-status')).to_contain_text('Cancelled')

    def test_native_resume_token_stays_out_of_rendered_demo_and_diagnostics(self):
        self.load({'nativeResumption': True})
        self.open_playground()
        self.page.get_by_role('checkbox',name='Allow this simulated session',exact=True).check()
        self.page.get_by_role('button',name='Simulate turn',exact=True).click()
        expect(self.page.locator('#session-status')).to_contain_text('Completed')
        self.page.get_by_role('button',name='Suspend native session',exact=True).click()
        expect(self.page.locator('#session-status')).to_have_text('Suspended')
        token='test-only-resume-token-sentinel'
        self.assertNotIn(token,self.page.locator('body').inner_text())
        self.assertNotIn(token,self.page.locator('body').inner_html())
        self.page.locator('[data-slot="sidebar"]').get_by_role('link',name='Diagnostics',exact=True).click()
        self.page.get_by_role('button',name='Refresh diagnostics',exact=True).click()
        self.assertNotIn(token,self.page.locator('body').inner_text())
        self.assertNotIn(token,self.page.locator('body').inner_html())
        self.page.locator('[data-slot="sidebar"]').get_by_role('link',name='Playground',exact=True).click()
        self.page.get_by_role('button',name='Resume native session',exact=True).click()
        expect(self.page.locator('#session-status')).to_have_text('Ready')
        self.page.get_by_role('button',name='Simulate turn',exact=True).click()
        expect(self.page.locator('#session-status')).to_have_text('Completed')
        self.assertEqual(self.page.evaluate('window.testNativeCalls'), ['sessions.create','turns.start','turns.start'])

    def test_failed_live_connection_never_keeps_fixture_accounts(self):
        self.load()
        self.page.get_by_role('button',name='Connect local runtime',exact=True).click()
        self.page.get_by_label('Runtime endpoint',exact=True).fill('http://127.0.0.1:9')
        self.page.get_by_label('App session token',exact=True).fill('synthetic-invalid-token')
        self.page.get_by_role('button',name='Connect runtime',exact=True).click()
        expect(self.page.locator('#mode-banner')).to_contain_text('Live')
        expect(self.page.locator('#connection-error')).to_contain_text('Unable to connect')
        expect(self.page.locator('[data-model-row]')).to_have_count(0)

    def test_keyboard_palette_and_search_preserve_focus(self):
        self.load()
        self.page.keyboard.press('Control+k')
        palette=self.page.locator('#palette-dialog')
        expect(palette).to_be_visible()
        # cmdk highlights the first result as soon as the list renders.
        palette.get_by_role('combobox').fill('Work workspace')
        expect(palette.get_by_role('option').first).to_be_visible()
        self.page.keyboard.press('Enter')
        expect(palette).not_to_be_visible()
        expect(self.page.locator('#selection-json')).to_contain_text('codex-work')
        search=self.page.locator('#catalog-query')
        search.fill('Precise')
        expect(search).to_be_focused()
        expect(self.page.get_by_test_id('model-codex-personal-0')).to_have_count(0)

    def test_mobile_drilldown_and_no_horizontal_page_overflow(self):
        self.page.set_viewport_size({'width':390,'height':844})
        self.load()
        self.page.locator('[data-variant="mobile"]').click()
        self.page.locator('#variant-view').get_by_role('button',name='Codex',exact=True).click()
        self.page.locator('#variant-view').get_by_role('button',name='Personal workspace',exact=True).click()
        self.page.get_by_test_id('model-codex-personal-0').click()
        expect(self.page.get_by_role('combobox',name='Reasoning effort',exact=True)).to_be_visible()
        self.assertTrue(self.page.evaluate('document.documentElement.scrollWidth <= window.innerWidth + 1'))

    def test_untrusted_model_name_is_text_not_markup(self):
        self.load({'maliciousLabel': True})
        self.assertIsNone(self.page.evaluate('globalThis.__unsafeExecuted'))
        expect(self.page.get_by_test_id('model-codex-personal-0').locator('img')).to_have_count(0)
        expect(self.page.get_by_test_id('model-codex-personal-0')).to_contain_text('<img')

    def test_large_catalog_has_bounded_render_and_search(self):
        self.load({'largeCatalog': True})
        self.assertLessEqual(self.page.locator('[data-model-row]').count(),60)
        self.page.locator('#catalog-query').fill('fixture/huge-4999')
        expect(self.page.get_by_test_id('huge-4999')).to_be_visible()


if __name__=='__main__':
    unittest.main()
