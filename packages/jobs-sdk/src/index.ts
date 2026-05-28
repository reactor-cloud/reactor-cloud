/**
 * @reactor/jobs-sdk
 *
 * Reactor Jobs SDK for building durable jobs with step checkpointing.
 *
 * @example
 * ```ts
 * import { createJobHandler } from '@reactor/jobs-sdk';
 *
 * export default createJobHandler(async (ctx) => {
 *   const user = await ctx.step('fetch-user', async () => {
 *     return await fetchUser(ctx.payload.userId);
 *   });
 *
 *   await ctx.step('send-email', async () => {
 *     return await sendEmail(user.email, 'Welcome!');
 *   });
 *
 *   await ctx.sleep('wait-24h', '24h');
 *
 *   await ctx.step('send-followup', async () => {
 *     return await sendEmail(user.email, 'How are you finding our service?');
 *   });
 *
 *   return { status: 'completed' };
 * });
 * ```
 */

export { createJobHandler, type JobHandler } from './handler.js';
export { type JobContext, type StepOptions } from './context.js';
export { type JobPayload, type StepResult } from './types.js';
export { JobSleepError, JobError } from './errors.js';
