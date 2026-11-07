# Read-only MSI inspection shared by packaging validation and manifest generation.
function Read-MsiTable {
    param([string]$Path, [string]$Query, [string[]]$Columns)
    $installer = $null; $database = $null; $view = $null; $record = $null
    try {
        $installer = New-Object -ComObject WindowsInstaller.Installer
        $database = $installer.OpenDatabase([IO.Path]::GetFullPath($Path), 0)
        $view = $database.OpenView($Query)
        [void]$view.Execute()
        while ($null -ne ($record = $view.Fetch())) {
            $row = [ordered]@{}
            for ($i = 0; $i -lt $Columns.Count; $i++) { $row[$Columns[$i]] = $record.StringData($i + 1) }
            [pscustomobject]$row
            [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($record)
            $record = $null
        }
    } finally {
        if ($view) { [void]$view.Close() }
        foreach ($item in @($record, $view, $database, $installer)) {
            if ($null -ne $item) { [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($item) }
        }
    }
}
