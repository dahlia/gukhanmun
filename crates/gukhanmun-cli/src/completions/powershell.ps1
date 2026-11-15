# Completion metadata and candidates come from the executable's clap schema.
# Tested with PowerShell 7 and Register-ArgumentCompleter's native interface.
& {
    $schema = ConvertFrom-Json @'
__GUKHANMUN_SCHEMA__
'@
    Register-ArgumentCompleter -Native -CommandName gukhanmun -ScriptBlock {
        param($wordToComplete, $commandAst, $cursorPosition)

        # Ignore arguments after the cursor, including when editing an earlier word.
        $elements = @($commandAst.CommandElements | Where-Object { $_.Extent.StartOffset -lt $cursorPosition })
        $current = $null
        $word = ''
        if ($elements.Count -gt 1 -and $elements[-1].Extent.EndOffset -ge $cursorPosition) {
            $current = $elements[-1]
            $word = $current.Extent.Text.Substring(0, $cursorPosition - $current.Extent.StartOffset)
        }
        # A value-taking short option consumes the remainder of its cluster.
        $resolveShort = {
            param($token)
            if (-not $token.StartsWith('-') -or $token.StartsWith('--')) { return $null }
            for ($index = 1; $index -lt $token.Length; $index++) {
                $name = '-' + $token[$index]
                $option = @($schema | Where-Object { $_.names -ccontains $name })
                if ($option.Count -eq 0) { return $null }
                if ($option[0].takes_value) {
                    $prefix = $token.Substring(0, $index + 1)
                    $value = $token.Substring($index + 1)
                    $attached = $value.Length -gt 0
                    if ($value.StartsWith('=')) {
                        $prefix += '='
                        $value = $value.Substring(1)
                    }
                    return @{
                        Option = $option[0]
                        Prefix = $prefix
                        Value = $value
                        Attached = $attached
                    }
                }
            }
            return $null
        }
        $pending = $null
        $terminated = $false
        foreach ($element in $elements | Select-Object -Skip 1) {
            if ($element -eq $current) { break }
            $token = $element.Extent.Text
            if ($token -ceq '--') {
                $terminated = $true
                $pending = $null
                continue
            }
            if ($terminated) { continue }
            if ($null -ne $pending) {
                $pending = $null
                continue
            }
            $option = @($schema | Where-Object { $_.names -ccontains $token })
            if ($option.Count -gt 0 -and $option[0].takes_value) {
                $pending = $option[0]
            } elseif ($option.Count -eq 0) {
                $short = & $resolveShort $token
                if ($null -ne $short -and -not $short.Attached) { $pending = $short.Option }
            }
        }

        $valueOption = $pending
        $prefix = ''
        if (-not $terminated -and $word -match '^([^=]+)=(.*)$') {
            $option = @($schema | Where-Object { $_.names -ccontains $Matches[1] })
            if ($option.Count -gt 0 -and $option[0].takes_value) {
                $valueOption = $option[0]
                $prefix = $Matches[1] + '='
                $word = $Matches[2]
            }
        }
        if (-not $terminated -and $null -eq $valueOption) {
            $short = & $resolveShort $word
            if ($null -ne $short -and $short.Attached) {
                $valueOption = $short.Option
                $prefix = $short.Prefix
                $word = $short.Value
            }
        }
        if ($null -ne $valueOption) {
            if ($valueOption.values.Count -gt 0) {
                $valuePrefix = $word.Trim("'`"")
                $matched = $false
                foreach ($value in $valueOption.values) {
                    if ($value.StartsWith($valuePrefix, [StringComparison]::Ordinal)) {
                        $matched = $true
                        $candidate = $prefix + $value
                        if ($candidate -match '[\s''"]') { $candidate = "'" + $candidate.Replace("'", "''") + "'" }
                        [System.Management.Automation.CompletionResult]::new(
                            $candidate, $value, 'ParameterValue', $value)
                    }
                }
                if ($matched) { return }
            } elseif ($valueOption.path) {
                foreach ($file in [System.Management.Automation.CompletionCompleters]::CompleteFilename($word)) {
                    [System.Management.Automation.CompletionResult]::new(
                        $prefix + $file.CompletionText, $file.ListItemText, $file.ResultType, $file.ToolTip)
                }
                return
            }
            # An empty pipeline enables PowerShell's filename fallback. Keep the
            # input unchanged when a non-path value has no candidates. The public
            # CompletionResult constructor rejects empty text, so an empty word
            # uses whitespace, which leaves the argument unfilled.
            $unchanged = if ($null -ne $current) { $current.Extent.Text } else { ' ' }
            [System.Management.Automation.CompletionResult]::new(
                $unchanged, $unchanged, 'ParameterValue', 'No completions')
            return
        }
        if (-not $terminated -and $word.StartsWith('-')) {
            foreach ($option in $schema) {
                foreach ($name in $option.names) {
                    if ($name.StartsWith($word, [StringComparison]::Ordinal)) {
                        $help = if ($option.help) { $option.help } else { $name }
                        [System.Management.Automation.CompletionResult]::new($name, $name, 'ParameterName', $help)
                    }
                }
            }
        } else {
            [System.Management.Automation.CompletionCompleters]::CompleteFilename($word)
        }
    }.GetNewClosure()
}
