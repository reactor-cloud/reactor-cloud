import { useState, useEffect } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import {
  Play,
  Square,
  RefreshCw,
  CheckCircle,
  XCircle,
  Clock,
  TrendingUp,
  BookOpen,
  Database,
  AlertCircle,
} from 'lucide-react'

interface BaselineResponse {
  passRate: number
  total: number
  passed: number
  failed: number
  passRatesByLevel: Record<string, number>
  durationMs: number
}

interface RunStatusResponse {
  running: boolean
  iteration: number | null
  passRate: number | null
}

interface LessonSummary {
  id: string
  title: string
  tier: string
  scope: string
  kind: string
  citations: number
  successRate: number
}

interface ReportSummary {
  iteration: number
  passRateBefore: number
  passRateAfter: number
  completedAt: string
}

interface TestResult {
  runId: string
  testId: string
  passed: boolean
  scores: Array<{
    passed: boolean
    scorerKind: string
    message: string
  }>
}

interface IterationComplete {
  iteration: number
  passRates: Record<string, number>
  promotions: string[]
  demotions: string[]
}

export function FoundryView() {
  const [isRunning, setIsRunning] = useState(false)
  const [baseline, setBaseline] = useState<BaselineResponse | null>(null)
  const [lessons, setLessons] = useState<LessonSummary[]>([])
  const [reports, setReports] = useState<ReportSummary[]>([])
  const [testStream, setTestStream] = useState<TestResult[]>([])
  const [currentIteration, setCurrentIteration] = useState<IterationComplete | null>(null)
  const [selectedLevels, setSelectedLevels] = useState(['L0', 'L1', 'L2'])
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    const unsubscribeRunStarted = listen<{ runId: string }>('foundry:run_started', (_event) => {
      setIsRunning(true)
      setTestStream([])
    })

    const unsubscribeTestResult = listen<TestResult>('foundry:test_result', (event) => {
      setTestStream((prev) => [...prev.slice(-50), event.payload])
    })

    const unsubscribeIterationComplete = listen<IterationComplete>('foundry:iteration_complete', (event) => {
      setCurrentIteration(event.payload)
    })

    const unsubscribeRunStopped = listen<{ runId: string; reason: string }>('foundry:run_stopped', (_event) => {
      setIsRunning(false)
      loadReports()
    })

    const unsubscribeError = listen<{ location: string; message: string }>('foundry:error', (event) => {
      setError(`${event.payload.location}: ${event.payload.message}`)
    })

    loadLessons()
    loadReports()
    checkStatus()

    return () => {
      unsubscribeRunStarted.then((fn) => fn())
      unsubscribeTestResult.then((fn) => fn())
      unsubscribeIterationComplete.then((fn) => fn())
      unsubscribeRunStopped.then((fn) => fn())
      unsubscribeError.then((fn) => fn())
    }
  }, [])

  async function checkStatus() {
    try {
      const status = await invoke<RunStatusResponse>('foundry_status')
      setIsRunning(status.running)
    } catch (err) {
      console.error('Failed to check status:', err)
    }
  }

  async function loadLessons() {
    try {
      const result = await invoke<LessonSummary[]>('foundry_lessons_list', {
        request: { tier: null, scope: null, domain: null },
      })
      setLessons(result)
    } catch (err) {
      console.error('Failed to load lessons:', err)
    }
  }

  async function loadReports() {
    try {
      const result = await invoke<ReportSummary[]>('foundry_reports_list')
      setReports(result)
    } catch (err) {
      console.error('Failed to load reports:', err)
    }
  }

  async function runBaseline() {
    setLoading(true)
    setError(null)
    try {
      const result = await invoke<BaselineResponse>('foundry_baseline', {
        request: { levels: selectedLevels, concurrency: 4 },
      })
      setBaseline(result)
    } catch (err) {
      setError(String(err))
    } finally {
      setLoading(false)
    }
  }

  async function startRun() {
    setError(null)
    try {
      await invoke('foundry_run', {
        request: { levels: selectedLevels, maxIterations: 10, concurrency: 4 },
      })
    } catch (err) {
      setError(String(err))
    }
  }

  async function stopRun() {
    try {
      await invoke('foundry_stop')
    } catch (err) {
      setError(String(err))
    }
  }

  const levelOptions = ['L0', 'L1', 'L2', 'L3', 'L4']

  return (
    <div className="h-full overflow-auto bg-neutral-950 text-neutral-100">
      <div className="max-w-6xl mx-auto p-6">
        <div className="flex items-center justify-between mb-6">
          <h1 className="text-2xl font-semibold flex items-center gap-2">
            <span className="text-amber-500">⚗️</span> Foundry
          </h1>
          <div className="flex items-center gap-2">
            {isRunning ? (
              <button
                onClick={stopRun}
                className="px-4 py-2 bg-red-600 hover:bg-red-700 rounded-lg flex items-center gap-2 transition-colors"
              >
                <Square className="w-4 h-4" />
                Stop
              </button>
            ) : (
              <>
                <button
                  onClick={runBaseline}
                  disabled={loading}
                  className="px-4 py-2 bg-neutral-700 hover:bg-neutral-600 rounded-lg flex items-center gap-2 transition-colors disabled:opacity-50"
                >
                  <RefreshCw className={`w-4 h-4 ${loading ? 'animate-spin' : ''}`} />
                  Baseline
                </button>
                <button
                  onClick={startRun}
                  className="px-4 py-2 bg-green-600 hover:bg-green-700 rounded-lg flex items-center gap-2 transition-colors"
                >
                  <Play className="w-4 h-4" />
                  Run Loop
                </button>
              </>
            )}
          </div>
        </div>

        {error && (
          <div className="mb-6 p-4 bg-red-900/30 border border-red-700 rounded-lg flex items-start gap-3">
            <AlertCircle className="w-5 h-5 text-red-500 flex-shrink-0 mt-0.5" />
            <p className="text-red-200">{error}</p>
          </div>
        )}

        {/* Level selector */}
        <div className="mb-6 p-4 bg-neutral-900 rounded-lg">
          <h3 className="text-sm font-medium text-neutral-400 mb-3">Test Levels</h3>
          <div className="flex gap-2 flex-wrap">
            {levelOptions.map((level) => (
              <button
                key={level}
                onClick={() => {
                  setSelectedLevels((prev) =>
                    prev.includes(level) ? prev.filter((l) => l !== level) : [...prev, level]
                  )
                }}
                className={`px-3 py-1.5 rounded-md text-sm transition-colors ${
                  selectedLevels.includes(level)
                    ? 'bg-amber-600 text-white'
                    : 'bg-neutral-800 text-neutral-400 hover:bg-neutral-700'
                }`}
              >
                {level}
              </button>
            ))}
          </div>
        </div>

        {/* Dashboard cards */}
        <div className="grid grid-cols-4 gap-4 mb-6">
          <DashboardCard
            title="Pass Rate"
            value={baseline ? `${(baseline.passRate * 100).toFixed(1)}%` : '-'}
            icon={TrendingUp}
            color="text-green-500"
          />
          <DashboardCard
            title="Tests"
            value={baseline ? `${baseline.passed}/${baseline.total}` : '-'}
            icon={CheckCircle}
            color="text-blue-500"
          />
          <DashboardCard
            title="Lessons"
            value={lessons.length.toString()}
            icon={BookOpen}
            color="text-amber-500"
          />
          <DashboardCard
            title="Iterations"
            value={reports.length.toString()}
            icon={Database}
            color="text-purple-500"
          />
        </div>

        {/* Pass rates by level */}
        {baseline && (
          <div className="mb-6 p-4 bg-neutral-900 rounded-lg">
            <h3 className="text-sm font-medium text-neutral-400 mb-3">Pass Rates by Level</h3>
            <div className="grid grid-cols-5 gap-3">
              {Object.entries(baseline.passRatesByLevel).map(([level, rate]) => (
                <div key={level} className="bg-neutral-800 rounded-lg p-3">
                  <div className="text-sm text-neutral-400 mb-1">{level}</div>
                  <div className="text-xl font-semibold">
                    {(rate * 100).toFixed(0)}%
                  </div>
                  <div className="mt-2 h-1.5 bg-neutral-700 rounded-full overflow-hidden">
                    <div
                      className="h-full bg-green-500 transition-all"
                      style={{ width: `${rate * 100}%` }}
                    />
                  </div>
                </div>
              ))}
            </div>
          </div>
        )}

        {/* Running tests stream */}
        {isRunning && testStream.length > 0 && (
          <div className="mb-6 p-4 bg-neutral-900 rounded-lg">
            <h3 className="text-sm font-medium text-neutral-400 mb-3">Test Stream</h3>
            <div className="max-h-48 overflow-y-auto space-y-1">
              {testStream.map((result, i) => (
                <div
                  key={`${result.testId}-${i}`}
                  className={`flex items-center gap-2 text-sm px-2 py-1 rounded ${
                    result.passed ? 'bg-green-900/20' : 'bg-red-900/20'
                  }`}
                >
                  {result.passed ? (
                    <CheckCircle className="w-4 h-4 text-green-500" />
                  ) : (
                    <XCircle className="w-4 h-4 text-red-500" />
                  )}
                  <span className="font-mono">{result.testId}</span>
                </div>
              ))}
            </div>
          </div>
        )}

        {/* Current iteration status */}
        {currentIteration && (
          <div className="mb-6 p-4 bg-neutral-900 rounded-lg">
            <h3 className="text-sm font-medium text-neutral-400 mb-3">
              Iteration {currentIteration.iteration}
            </h3>
            <div className="grid grid-cols-3 gap-4">
              <div>
                <div className="text-sm text-neutral-400">Overall Pass Rate</div>
                <div className="text-lg font-semibold">
                  {((currentIteration.passRates?.overall ?? 0) * 100).toFixed(1)}%
                </div>
              </div>
              <div>
                <div className="text-sm text-neutral-400">Promotions</div>
                <div className="text-lg font-semibold text-green-500">
                  {currentIteration.promotions?.length ?? 0}
                </div>
              </div>
              <div>
                <div className="text-sm text-neutral-400">Demotions</div>
                <div className="text-lg font-semibold text-red-500">
                  {currentIteration.demotions?.length ?? 0}
                </div>
              </div>
            </div>
          </div>
        )}

        {/* Lessons list */}
        <div className="mb-6">
          <h3 className="text-lg font-medium mb-3 flex items-center gap-2">
            <BookOpen className="w-5 h-5 text-amber-500" />
            Lessons
          </h3>
          {lessons.length === 0 ? (
            <div className="p-8 bg-neutral-900 rounded-lg text-center text-neutral-500">
              No lessons yet. Run the auto-iteration loop to generate lessons from failures.
            </div>
          ) : (
            <div className="bg-neutral-900 rounded-lg overflow-hidden">
              <table className="w-full">
                <thead className="bg-neutral-800">
                  <tr>
                    <th className="px-4 py-2 text-left text-sm font-medium text-neutral-400">Title</th>
                    <th className="px-4 py-2 text-left text-sm font-medium text-neutral-400">Tier</th>
                    <th className="px-4 py-2 text-left text-sm font-medium text-neutral-400">Kind</th>
                    <th className="px-4 py-2 text-left text-sm font-medium text-neutral-400">Citations</th>
                    <th className="px-4 py-2 text-left text-sm font-medium text-neutral-400">Success Rate</th>
                  </tr>
                </thead>
                <tbody>
                  {lessons.map((lesson) => (
                    <tr key={lesson.id} className="border-t border-neutral-800 hover:bg-neutral-800/50">
                      <td className="px-4 py-2 text-sm">{lesson.title}</td>
                      <td className="px-4 py-2">
                        <span className={`px-2 py-0.5 rounded text-xs font-medium ${getTierColor(lesson.tier)}`}>
                          {lesson.tier}
                        </span>
                      </td>
                      <td className="px-4 py-2 text-sm text-neutral-400">{lesson.kind}</td>
                      <td className="px-4 py-2 text-sm">{lesson.citations}</td>
                      <td className="px-4 py-2 text-sm">{(lesson.successRate * 100).toFixed(0)}%</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}
        </div>

        {/* Recent reports */}
        <div>
          <h3 className="text-lg font-medium mb-3 flex items-center gap-2">
            <Clock className="w-5 h-5 text-purple-500" />
            Recent Iterations
          </h3>
          {reports.length === 0 ? (
            <div className="p-8 bg-neutral-900 rounded-lg text-center text-neutral-500">
              No iterations yet. Start a run to see iteration reports.
            </div>
          ) : (
            <div className="bg-neutral-900 rounded-lg overflow-hidden">
              <table className="w-full">
                <thead className="bg-neutral-800">
                  <tr>
                    <th className="px-4 py-2 text-left text-sm font-medium text-neutral-400">Iteration</th>
                    <th className="px-4 py-2 text-left text-sm font-medium text-neutral-400">Before</th>
                    <th className="px-4 py-2 text-left text-sm font-medium text-neutral-400">After</th>
                    <th className="px-4 py-2 text-left text-sm font-medium text-neutral-400">Change</th>
                    <th className="px-4 py-2 text-left text-sm font-medium text-neutral-400">Completed</th>
                  </tr>
                </thead>
                <tbody>
                  {reports.map((report) => {
                    const change = report.passRateAfter - report.passRateBefore
                    return (
                      <tr key={report.iteration} className="border-t border-neutral-800 hover:bg-neutral-800/50">
                        <td className="px-4 py-2 text-sm font-mono">#{report.iteration}</td>
                        <td className="px-4 py-2 text-sm">{(report.passRateBefore * 100).toFixed(1)}%</td>
                        <td className="px-4 py-2 text-sm">{(report.passRateAfter * 100).toFixed(1)}%</td>
                        <td className="px-4 py-2 text-sm">
                          <span className={change >= 0 ? 'text-green-500' : 'text-red-500'}>
                            {change >= 0 ? '+' : ''}{(change * 100).toFixed(1)}%
                          </span>
                        </td>
                        <td className="px-4 py-2 text-sm text-neutral-400">
                          {new Date(report.completedAt).toLocaleString()}
                        </td>
                      </tr>
                    )
                  })}
                </tbody>
              </table>
            </div>
          )}
        </div>
      </div>
    </div>
  )
}

function DashboardCard({
  title,
  value,
  icon: Icon,
  color,
}: {
  title: string
  value: string
  icon: React.ComponentType<{ className?: string }>
  color: string
}) {
  return (
    <div className="bg-neutral-900 rounded-lg p-4">
      <div className="flex items-center justify-between mb-2">
        <span className="text-sm text-neutral-400">{title}</span>
        <Icon className={`w-5 h-5 ${color}`} />
      </div>
      <div className="text-2xl font-semibold">{value}</div>
    </div>
  )
}

function getTierColor(tier: string): string {
  switch (tier) {
    case 'T0':
      return 'bg-neutral-700 text-neutral-300'
    case 'T1':
      return 'bg-blue-900 text-blue-300'
    case 'T2':
      return 'bg-green-900 text-green-300'
    case 'T3':
      return 'bg-amber-900 text-amber-300'
    case 'T4':
      return 'bg-purple-900 text-purple-300'
    default:
      return 'bg-neutral-700 text-neutral-300'
  }
}
