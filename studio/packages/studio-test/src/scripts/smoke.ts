import { defineTest } from '../harness.js';

export default defineTest({
  name: 'smoke',
  description: 'Basic smoke test to verify devserver and test harness are working',

  setup: {
    agent: 'coder',
  },

  async run(s) {
    s.log('Starting smoke test');

    // Take initial screenshot
    await s.screenshot('initial');

    // Send a simple message
    s.log('Sending hello message...');
    const result = await s.sendAndWait('Hello! Please respond with a greeting.', {
      timeoutMs: 60_000,
    });

    // Assertions
    s.assert('message_sent', result.success, true, result.success);
    s.assert('has_response', !!result.finalText, true, !!result.finalText);
    const isGreeting = Boolean(
      result.finalText?.toLowerCase().includes('hello') ||
        result.finalText?.toLowerCase().includes('hi') ||
        result.finalText?.toLowerCase().includes('hey')
    );
    s.assert('response_is_greeting', isGreeting, true, result.finalText?.slice(0, 100));

    // Log response info
    s.log('Response received', {
      text: result.finalText?.slice(0, 200),
      toolCount: result.toolSequence.length,
      durationMs: result.durationMs,
    });

    // Take final screenshot
    await s.screenshot('final');

    return {
      data: {
        responseText: result.finalText,
        toolCount: result.toolSequence.length,
        durationMs: result.durationMs,
      },
    };
  },
});
