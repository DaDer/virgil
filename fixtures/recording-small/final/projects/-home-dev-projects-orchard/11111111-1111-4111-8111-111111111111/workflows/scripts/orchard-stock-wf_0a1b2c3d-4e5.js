export const meta = {
  name: 'orchard-stock',
  description: 'Plan and build the stock adjustment endpoints, screen and tests',
  phases: [
    { title: 'Plan', detail: 'one outline for everybody', model: 'opus' },
    { title: 'Build', detail: 'api, screen and tests in parallel' },
  ],
}

phase('Plan')
const plan = await agent(`Outline the stock adjustment work for ${args.area}.`, { label: 'plan:outline', phase: 'Plan' })
phase('Build')
const built = await parallel([
  () => agent('Build the adjustment endpoint. Plan: ' + plan, { label: 'build:api', phase: 'Build' }),
  () => agent('Build the adjustment form. Plan: ' + plan, { label: 'build:ui', phase: 'Build' }),
])
const tests = await agent('Cover the endpoint with tests.', { label: 'build:tests', phase: 'Build', model: 'sonnet' })
return { built, tests }
