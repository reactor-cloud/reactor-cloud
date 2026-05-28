import { useState, useEffect, useMemo } from 'react'

type Theme = 'light' | 'dark' | 'system'
type ResolvedTheme = 'light' | 'dark'

export function useTheme() {
  const [theme, setThemeState] = useState<Theme>(() => {
    if (typeof window !== 'undefined') {
      return (localStorage.getItem('theme') as Theme) || 'system'
    }
    return 'system'
  })
  
  const [systemTheme, setSystemTheme] = useState<ResolvedTheme>(() => {
    if (typeof window !== 'undefined') {
      return window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light'
    }
    return 'light'
  })

  useEffect(() => {
    const root = document.documentElement
    
    const applyTheme = (t: Theme) => {
      if (t === 'system') {
        const systemDark = window.matchMedia('(prefers-color-scheme: dark)').matches
        root.classList.toggle('dark', systemDark)
        root.classList.toggle('light', !systemDark)
      } else {
        root.classList.toggle('dark', t === 'dark')
        root.classList.toggle('light', t === 'light')
      }
    }

    applyTheme(theme)

    if (theme === 'system') {
      const mediaQuery = window.matchMedia('(prefers-color-scheme: dark)')
      const handler = () => {
        const isDark = mediaQuery.matches
        setSystemTheme(isDark ? 'dark' : 'light')
        applyTheme('system')
      }
      mediaQuery.addEventListener('change', handler)
      return () => mediaQuery.removeEventListener('change', handler)
    }
  }, [theme])

  const setTheme = (t: Theme) => {
    localStorage.setItem('theme', t)
    setThemeState(t)
  }

  const resolvedTheme: ResolvedTheme = useMemo(() => {
    if (theme === 'system') {
      return systemTheme
    }
    return theme
  }, [theme, systemTheme])

  return { theme, setTheme, resolvedTheme }
}
