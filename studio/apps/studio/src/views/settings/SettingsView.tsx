import { useState, useEffect } from 'react'
import { cn } from '@/lib/utils'
import { Button, Input, ScrollArea } from '@/components/ui'
import { Key, Check, X, Eye, EyeOff, Loader2 } from 'lucide-react'
import { credentialList, credentialSet, credentialDelete, type CredentialInfo } from '@/lib/ipc'
import type { ViewProps } from '@/views/types'

interface ProviderConfig {
  key: string
  label: string
  description: string
  placeholder: string
}

const PROVIDERS: ProviderConfig[] = [
  {
    key: 'openrouter_api_key',
    label: 'OpenRouter',
    description: 'Access multiple LLM providers through OpenRouter',
    placeholder: 'sk-or-...',
  },
  {
    key: 'openai_api_key',
    label: 'OpenAI',
    description: 'Direct OpenAI API access',
    placeholder: 'sk-...',
  },
]

export function SettingsView({ viewRef: _viewRef }: ViewProps) {
  const [credentials, setCredentials] = useState<CredentialInfo[]>([])
  const [loading, setLoading] = useState(true)
  const [saving, setSaving] = useState<string | null>(null)

  useEffect(() => {
    loadCredentials()
  }, [])

  async function loadCredentials() {
    try {
      const list = await credentialList()
      setCredentials(list)
    } catch (err) {
      console.error('Failed to load credentials:', err)
    } finally {
      setLoading(false)
    }
  }

  function isSet(key: string): boolean {
    return credentials.find((c) => c.key === key)?.isSet ?? false
  }

  if (loading) {
    return (
      <div className="flex items-center justify-center h-full">
        <Loader2 className="w-6 h-6 animate-spin text-[var(--text-tertiary)]" />
      </div>
    )
  }

  return (
    <ScrollArea className="h-full">
      <div className="p-6 max-w-2xl mx-auto space-y-8">
        <div>
          <h1 className="text-2xl font-bold text-[var(--text-primary)]">Settings</h1>
          <p className="text-sm text-[var(--text-secondary)] mt-1">
            Configure your AI providers and preferences
          </p>
        </div>

        <section>
          <h2 className="text-lg font-semibold text-[var(--text-primary)] mb-4">
            Providers
          </h2>
          <div className="space-y-4">
            {PROVIDERS.map((provider) => (
              <ProviderCard
                key={provider.key}
                provider={provider}
                isSet={isSet(provider.key)}
                isSaving={saving === provider.key}
                onSave={async (value) => {
                  setSaving(provider.key)
                  try {
                    await credentialSet(provider.key, value)
                    await loadCredentials()
                  } finally {
                    setSaving(null)
                  }
                }}
                onDelete={async () => {
                  setSaving(provider.key)
                  try {
                    await credentialDelete(provider.key)
                    await loadCredentials()
                  } finally {
                    setSaving(null)
                  }
                }}
              />
            ))}
          </div>
        </section>
      </div>
    </ScrollArea>
  )
}

interface ProviderCardProps {
  provider: ProviderConfig
  isSet: boolean
  isSaving: boolean
  onSave: (value: string) => Promise<void>
  onDelete: () => Promise<void>
}

function ProviderCard({ provider, isSet, isSaving, onSave, onDelete }: ProviderCardProps) {
  const [editing, setEditing] = useState(false)
  const [value, setValue] = useState('')
  const [showValue, setShowValue] = useState(false)

  const handleSave = async () => {
    if (!value.trim()) return
    await onSave(value)
    setValue('')
    setEditing(false)
  }

  const handleCancel = () => {
    setValue('')
    setEditing(false)
  }

  return (
    <div
      className={cn(
        'rounded-lg border border-[var(--border)] p-4',
        'bg-[var(--background-secondary)]'
      )}
    >
      <div className="flex items-start gap-3">
        <div
          className={cn(
            'w-10 h-10 rounded-lg flex items-center justify-center',
            isSet ? 'bg-green-500/20' : 'bg-[var(--background-tertiary)]'
          )}
        >
          <Key
            className={cn(
              'w-5 h-5',
              isSet ? 'text-green-400' : 'text-[var(--text-tertiary)]'
            )}
          />
        </div>

        <div className="flex-1 min-w-0">
          <div className="flex items-center gap-2">
            <h3 className="font-medium text-[var(--text-primary)]">
              {provider.label}
            </h3>
            {isSet && (
              <span className="inline-flex items-center gap-1 px-2 py-0.5 rounded-full bg-green-500/20 text-green-400 text-xs">
                <Check className="w-3 h-3" />
                Configured
              </span>
            )}
          </div>
          <p className="text-sm text-[var(--text-secondary)] mt-0.5">
            {provider.description}
          </p>

          {editing ? (
            <div className="mt-3 space-y-2">
              <div className="relative">
                <Input
                  type={showValue ? 'text' : 'password'}
                  value={value}
                  onChange={(e) => setValue(e.target.value)}
                  placeholder={provider.placeholder}
                  className="pr-10"
                />
                <button
                  type="button"
                  onClick={() => setShowValue(!showValue)}
                  className="absolute right-2 top-1/2 -translate-y-1/2 p-1 text-[var(--text-tertiary)] hover:text-[var(--text-secondary)]"
                >
                  {showValue ? (
                    <EyeOff className="w-4 h-4" />
                  ) : (
                    <Eye className="w-4 h-4" />
                  )}
                </button>
              </div>
              <div className="flex gap-2">
                <Button
                  variant="default"
                  size="sm"
                  onClick={handleSave}
                  disabled={!value.trim() || isSaving}
                >
                  {isSaving ? (
                    <Loader2 className="w-4 h-4 animate-spin" />
                  ) : (
                    'Save'
                  )}
                </Button>
                <Button variant="ghost" size="sm" onClick={handleCancel}>
                  Cancel
                </Button>
              </div>
            </div>
          ) : (
            <div className="mt-3 flex gap-2">
              <Button
                variant="secondary"
                size="sm"
                onClick={() => setEditing(true)}
              >
                {isSet ? 'Update' : 'Configure'}
              </Button>
              {isSet && (
                <Button
                  variant="ghost"
                  size="sm"
                  onClick={onDelete}
                  disabled={isSaving}
                  className="text-red-400 hover:text-red-300"
                >
                  {isSaving ? (
                    <Loader2 className="w-4 h-4 animate-spin" />
                  ) : (
                    <X className="w-4 h-4" />
                  )}
                </Button>
              )}
            </div>
          )}
        </div>
      </div>
    </div>
  )
}
