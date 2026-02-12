import { useState, useEffect } from 'react'
import { settingsApi } from '@/api'
import type {
  Settings,
  JellyfinSettings,
  WhisperSettings,
  TranslationSettings,
  NotificationSettings,
  TestConnectionResponse,
} from '@/api'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Button } from '@/components/ui/button'
import { Server, Mic, Languages, Bell, Loader2, Check, X } from 'lucide-react'

export function SettingsPage() {
  const [settings, setSettings] = useState<Settings | null>(null)
  const [loading, setLoading] = useState(true)
  const [saving, setSaving] = useState(false)
  const [testResult, setTestResult] = useState<TestConnectionResponse | null>(null)
  const [testing, setTesting] = useState(false)

  useEffect(() => {
    async function fetchSettings() {
      try {
        const response = await settingsApi.get()
        setSettings(response)
      } catch (error) {
        console.error('Failed to fetch settings:', error)
      } finally {
        setLoading(false)
      }
    }
    fetchSettings()
  }, [])

  const handleSave = async () => {
    if (!settings) return
    setSaving(true)
    try {
      await settingsApi.update(settings)
      // TODO: Show success toast
    } catch (error) {
      console.error('Failed to save settings:', error)
    } finally {
      setSaving(false)
    }
  }

  const handleTestJellyfin = async () => {
    if (!settings) return
    setTesting(true)
    setTestResult(null)
    try {
      const result = await settingsApi.testJellyfin(settings.jellyfin)
      setTestResult(result)
    } catch (error) {
      setTestResult({ success: false, message: 'Connection failed', details: {} })
    } finally {
      setTesting(false)
    }
  }

  const handleTestNotification = async () => {
    if (!settings) return
    setTesting(true)
    setTestResult(null)
    try {
      const result = await settingsApi.testNotification(settings.notifications)
      setTestResult(result)
    } catch (error) {
      setTestResult({ success: false, message: 'Test failed', details: {} })
    } finally {
      setTesting(false)
    }
  }

  const updateJellyfin = (field: keyof JellyfinSettings, value: string) => {
    if (!settings) return
    setSettings({
      ...settings,
      jellyfin: { ...settings.jellyfin, [field]: value },
    })
  }

  const updateWhisper = (field: keyof WhisperSettings, value: string) => {
    if (!settings) return
    setSettings({
      ...settings,
      whisper: { ...settings.whisper, [field]: value },
    })
  }

  const updateTranslation = (field: keyof TranslationSettings, value: string) => {
    if (!settings) return
    setSettings({
      ...settings,
      translation: { ...settings.translation, [field]: value },
    })
  }

  const updateNotification = (
    field: keyof NotificationSettings,
    value: string | string[] | null
  ) => {
    if (!settings) return
    setSettings({
      ...settings,
      notifications: { ...settings.notifications, [field]: value },
    })
  }

  const handleTabChange = () => {
    setTestResult(null)
  }

  if (loading) {
    return (
      <div className="flex items-center justify-center py-12">
        <Loader2 className="h-8 w-8 animate-spin text-muted-foreground" />
      </div>
    )
  }

  if (!settings) {
    return (
      <div className="text-destructive text-center py-12">Failed to load settings</div>
    )
  }

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold">Settings</h1>
          <p className="text-muted-foreground mt-1">Configure your Submate instance</p>
        </div>
        <Button onClick={handleSave} disabled={saving}>
          {saving ? (
            <>
              <Loader2 className="mr-2 h-4 w-4 animate-spin" />
              Saving...
            </>
          ) : (
            'Save Changes'
          )}
        </Button>
      </div>

      {/* Test Result Alert */}
      {testResult && (
        <div
          className={`flex items-center gap-3 p-4 rounded-lg border ${
            testResult.success
              ? 'bg-green-950/50 border-green-800 text-green-400'
              : 'bg-red-950/50 border-red-800 text-red-400'
          }`}
        >
          {testResult.success ? (
            <Check className="h-5 w-5 flex-shrink-0" />
          ) : (
            <X className="h-5 w-5 flex-shrink-0" />
          )}
          <p>
            <span className="font-medium">{testResult.success ? 'Success:' : 'Error:'}</span>{' '}
            {testResult.message}
          </p>
        </div>
      )}

      {/* Settings Tabs */}
      <Tabs defaultValue="jellyfin" onValueChange={handleTabChange}>
        <TabsList className="grid w-full grid-cols-4">
          <TabsTrigger value="jellyfin" className="gap-2">
            <Server className="h-4 w-4" />
            <span className="hidden sm:inline">Jellyfin</span>
          </TabsTrigger>
          <TabsTrigger value="whisper" className="gap-2">
            <Mic className="h-4 w-4" />
            <span className="hidden sm:inline">Whisper</span>
          </TabsTrigger>
          <TabsTrigger value="translation" className="gap-2">
            <Languages className="h-4 w-4" />
            <span className="hidden sm:inline">Translation</span>
          </TabsTrigger>
          <TabsTrigger value="notifications" className="gap-2">
            <Bell className="h-4 w-4" />
            <span className="hidden sm:inline">Notifications</span>
          </TabsTrigger>
        </TabsList>

        <TabsContent value="jellyfin">
          <Card>
            <CardHeader>
              <CardTitle>Jellyfin Connection</CardTitle>
              <CardDescription>
                Configure your Jellyfin server connection for automatic subtitle processing
              </CardDescription>
            </CardHeader>
            <CardContent className="space-y-4">
              <div className="space-y-2">
                <Label htmlFor="jellyfin-url">Server URL</Label>
                <Input
                  id="jellyfin-url"
                  value={settings.jellyfin.server_url}
                  onChange={(e) => updateJellyfin('server_url', e.target.value)}
                  placeholder="http://jellyfin:8096"
                />
              </div>
              <div className="space-y-2">
                <Label htmlFor="jellyfin-api-key">API Key</Label>
                <Input
                  id="jellyfin-api-key"
                  type="password"
                  value={settings.jellyfin.api_key}
                  onChange={(e) => updateJellyfin('api_key', e.target.value)}
                  placeholder="Your Jellyfin API key"
                />
              </div>
              <Button variant="secondary" onClick={handleTestJellyfin} disabled={testing}>
                {testing ? (
                  <>
                    <Loader2 className="mr-2 h-4 w-4 animate-spin" />
                    Testing...
                  </>
                ) : (
                  'Test Connection'
                )}
              </Button>
            </CardContent>
          </Card>
        </TabsContent>

        <TabsContent value="whisper">
          <Card>
            <CardHeader>
              <CardTitle>Whisper Settings</CardTitle>
              <CardDescription>
                Configure the Whisper model for speech-to-text transcription
              </CardDescription>
            </CardHeader>
            <CardContent className="space-y-4">
              <div className="space-y-2">
                <Label htmlFor="whisper-model">Model</Label>
                <select
                  id="whisper-model"
                  value={settings.whisper.model}
                  onChange={(e) => updateWhisper('model', e.target.value)}
                  className="flex h-10 w-full rounded-md border border-input bg-background px-3 py-2 text-sm ring-offset-background focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2"
                >
                  <option value="tiny">Tiny (fastest, least accurate)</option>
                  <option value="base">Base</option>
                  <option value="small">Small</option>
                  <option value="medium">Medium (recommended)</option>
                  <option value="large">Large (most accurate, slowest)</option>
                </select>
              </div>
              <div className="space-y-2">
                <Label htmlFor="whisper-device">Device</Label>
                <select
                  id="whisper-device"
                  value={settings.whisper.device}
                  onChange={(e) => updateWhisper('device', e.target.value)}
                  className="flex h-10 w-full rounded-md border border-input bg-background px-3 py-2 text-sm ring-offset-background focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2"
                >
                  <option value="auto">Auto (detect GPU)</option>
                  <option value="cpu">CPU</option>
                  <option value="cuda">CUDA (NVIDIA GPU)</option>
                </select>
              </div>
              <div className="space-y-2">
                <Label htmlFor="whisper-compute-type">Compute Type</Label>
                <select
                  id="whisper-compute-type"
                  value={settings.whisper.compute_type}
                  onChange={(e) => updateWhisper('compute_type', e.target.value)}
                  className="flex h-10 w-full rounded-md border border-input bg-background px-3 py-2 text-sm ring-offset-background focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2"
                >
                  <option value="int8">INT8 (fastest)</option>
                  <option value="float16">Float16</option>
                  <option value="float32">Float32 (most accurate)</option>
                </select>
              </div>
            </CardContent>
          </Card>
        </TabsContent>

        <TabsContent value="translation">
          <Card>
            <CardHeader>
              <CardTitle>Translation Settings</CardTitle>
              <CardDescription>
                Configure the LLM backend for subtitle translation
              </CardDescription>
            </CardHeader>
            <CardContent className="space-y-4">
              <div className="space-y-2">
                <Label htmlFor="translation-backend">Backend</Label>
                <select
                  id="translation-backend"
                  value={settings.translation.backend}
                  onChange={(e) => updateTranslation('backend', e.target.value)}
                  className="flex h-10 w-full rounded-md border border-input bg-background px-3 py-2 text-sm ring-offset-background focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2"
                >
                  <option value="ollama">Ollama (local, free)</option>
                  <option value="openai">OpenAI</option>
                  <option value="anthropic">Anthropic Claude</option>
                  <option value="gemini">Google Gemini</option>
                </select>
              </div>

              {settings.translation.backend === 'ollama' && (
                <>
                  <div className="space-y-2">
                    <Label htmlFor="ollama-url">Ollama URL</Label>
                    <Input
                      id="ollama-url"
                      value={settings.translation.ollama_url}
                      onChange={(e) => updateTranslation('ollama_url', e.target.value)}
                      placeholder="http://localhost:11434"
                    />
                  </div>
                  <div className="space-y-2">
                    <Label htmlFor="ollama-model">Ollama Model</Label>
                    <Input
                      id="ollama-model"
                      value={settings.translation.ollama_model}
                      onChange={(e) => updateTranslation('ollama_model', e.target.value)}
                      placeholder="llama2"
                    />
                  </div>
                </>
              )}

              {settings.translation.backend === 'openai' && (
                <>
                  <div className="space-y-2">
                    <Label htmlFor="openai-api-key">OpenAI API Key</Label>
                    <Input
                      id="openai-api-key"
                      type="password"
                      value={settings.translation.openai_api_key}
                      onChange={(e) => updateTranslation('openai_api_key', e.target.value)}
                    />
                  </div>
                  <div className="space-y-2">
                    <Label htmlFor="openai-model">OpenAI Model</Label>
                    <Input
                      id="openai-model"
                      value={settings.translation.openai_model}
                      onChange={(e) => updateTranslation('openai_model', e.target.value)}
                      placeholder="gpt-4"
                    />
                  </div>
                </>
              )}

              {settings.translation.backend === 'anthropic' && (
                <>
                  <div className="space-y-2">
                    <Label htmlFor="anthropic-api-key">Anthropic API Key</Label>
                    <Input
                      id="anthropic-api-key"
                      type="password"
                      value={settings.translation.anthropic_api_key}
                      onChange={(e) => updateTranslation('anthropic_api_key', e.target.value)}
                    />
                  </div>
                  <div className="space-y-2">
                    <Label htmlFor="claude-model">Claude Model</Label>
                    <Input
                      id="claude-model"
                      value={settings.translation.claude_model}
                      onChange={(e) => updateTranslation('claude_model', e.target.value)}
                      placeholder="claude-3-sonnet-20240229"
                    />
                  </div>
                </>
              )}

              {settings.translation.backend === 'gemini' && (
                <>
                  <div className="space-y-2">
                    <Label htmlFor="gemini-api-key">Gemini API Key</Label>
                    <Input
                      id="gemini-api-key"
                      type="password"
                      value={settings.translation.gemini_api_key}
                      onChange={(e) => updateTranslation('gemini_api_key', e.target.value)}
                    />
                  </div>
                  <div className="space-y-2">
                    <Label htmlFor="gemini-model">Gemini Model</Label>
                    <Input
                      id="gemini-model"
                      value={settings.translation.gemini_model}
                      onChange={(e) => updateTranslation('gemini_model', e.target.value)}
                      placeholder="gemini-pro"
                    />
                  </div>
                </>
              )}
            </CardContent>
          </Card>
        </TabsContent>

        <TabsContent value="notifications">
          <Card>
            <CardHeader>
              <CardTitle>Notification Settings</CardTitle>
              <CardDescription>
                Configure webhooks and notification services
              </CardDescription>
            </CardHeader>
            <CardContent className="space-y-4">
              <div className="space-y-2">
                <Label htmlFor="webhook-url">Webhook URL</Label>
                <Input
                  id="webhook-url"
                  value={settings.notifications.webhook_url || ''}
                  onChange={(e) => updateNotification('webhook_url', e.target.value || null)}
                  placeholder="https://your-webhook.url/endpoint"
                />
              </div>
              <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
                <div className="space-y-2">
                  <Label htmlFor="ntfy-url">ntfy URL</Label>
                  <Input
                    id="ntfy-url"
                    value={settings.notifications.ntfy_url || ''}
                    onChange={(e) => updateNotification('ntfy_url', e.target.value || null)}
                    placeholder="https://ntfy.sh"
                  />
                </div>
                <div className="space-y-2">
                  <Label htmlFor="ntfy-topic">ntfy Topic</Label>
                  <Input
                    id="ntfy-topic"
                    value={settings.notifications.ntfy_topic || ''}
                    onChange={(e) => updateNotification('ntfy_topic', e.target.value || null)}
                    placeholder="submate"
                  />
                </div>
              </div>
              <Button variant="secondary" onClick={handleTestNotification} disabled={testing}>
                {testing ? (
                  <>
                    <Loader2 className="mr-2 h-4 w-4 animate-spin" />
                    Testing...
                  </>
                ) : (
                  'Send Test Notification'
                )}
              </Button>
            </CardContent>
          </Card>
        </TabsContent>
      </Tabs>
    </div>
  )
}
