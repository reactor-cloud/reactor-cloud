import { cn } from '@/lib/utils'
import { Bot, Code, Lightbulb, Search, Sparkles, Cog, Wrench, FileText } from 'lucide-react'
import type { Agent } from '@/hooks/useAgents'

const iconMap: Record<string, React.ComponentType<{ className?: string; style?: React.CSSProperties }>> = {
  bot: Bot,
  code: Code,
  lightbulb: Lightbulb,
  search: Search,
  sparkles: Sparkles,
  cog: Cog,
  wrench: Wrench,
  'file-text': FileText,
  coder: Code,
  planner: Lightbulb,
  researcher: Search,
}

interface AgentIconProps {
  agent: Agent
  size?: 'sm' | 'md' | 'lg'
  className?: string
}

const sizeClasses = {
  sm: 'w-4 h-4 text-sm',
  md: 'w-5 h-5 text-base',
  lg: 'w-6 h-6 text-lg',
}

export function AgentIcon({ agent, size = 'sm', className }: AgentIconProps) {
  const sizeClass = sizeClasses[size]
  const iconKey = agent.avatar || agent.type || agent.id

  if (agent.avatar && /^\p{Emoji}/u.test(agent.avatar)) {
    return (
      <span
        className={cn(sizeClass, 'flex items-center justify-center', className)}
        style={{ color: agent.color }}
      >
        {agent.avatar}
      </span>
    )
  }

  const Icon = iconMap[iconKey] || Bot
  return (
    <Icon
      className={cn(sizeClass, className)}
      style={{ color: agent.color }}
    />
  )
}

interface LegacyAgentIconProps {
  icon?: string
  color: string
  size?: 'sm' | 'md' | 'lg'
  className?: string
}

export function LegacyAgentIcon({ icon, color, size = 'md', className }: LegacyAgentIconProps) {
  const Icon = icon && iconMap[icon] ? iconMap[icon] : Bot

  const legacySizeClasses = {
    sm: 'w-5 h-5 p-0.5',
    md: 'w-8 h-8 p-1',
    lg: 'w-10 h-10 p-1.5',
  }

  return (
    <div
      className={cn(
        'rounded-full flex items-center justify-center',
        legacySizeClasses[size],
        className
      )}
      style={{ backgroundColor: color }}
    >
      <Icon className="w-full h-full text-white" />
    </div>
  )
}
