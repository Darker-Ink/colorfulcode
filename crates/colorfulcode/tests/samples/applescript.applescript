-- Archive old files from the Desktop
(* Settings
   (* nested *) are below *)
property maxAge : 30
global archived

on run
	set archived to 0
	tell application "Finder"
		set oldFiles to every file of desktop whose modification date < ((current date) - maxAge * days)
		repeat with f in oldFiles
			my archiveFile(f)
		end repeat
	end tell
	display dialog "Archived " & archived & " files" buttons {"OK"} default button 1
end run

on archiveFile(theFile)
	try
		tell application "Finder" to move theFile to folder "Archive" of desktop
		set archived to archived + 1
	on error errMsg number errNum
		log "Failed: " & errMsg & " (" & errNum & ")"
	end try
end archiveFile

to formatSize(bytes)
	if bytes > 1.0E+6 then
		return ((bytes / 1000000) as integer) as text & " MB"
	else if bytes is missing value then
		return "unknown"
	end if
	return (bytes as text) & " bytes"
end formatSize

# Show a notification with a "quoted" word
display notification "Done" with title "Archive \"Helper\""
