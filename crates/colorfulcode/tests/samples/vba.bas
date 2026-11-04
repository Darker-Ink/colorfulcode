Attribute VB_Name = "ReportModule"
Option Explicit

' Builds a summary sheet from raw data
Private Const MAX_ROWS As Long = 10000

Private Type Point
    X As LongPtr
End Type

Public Sub BuildSummary()
    Dim ws As Worksheet
    Dim lastRow As Long, i As Long
    Dim total As Double
    Dim names As Collection

    Set ws = ThisWorkbook.Worksheets("Data")
    Set names = New Collection
    lastRow = ws.Cells(ws.Rows.Count, 1).End(xlUp).Row

    On Error GoTo Fail
    For i = 2 To lastRow
        If Not IsEmpty(ws.Cells(i, 3).Value) Then
            total = total + ws.Cells(i, 3).Value
        End If
    Next i

    Do While total > MAX_ROWS
        total = total / 2
    Loop

    MsgBox "Total: " & Format(total, "0.00") & vbCrLf & "Done", vbInformation
    Exit Sub

Fail:
    Debug.Print "Error " & Err.Number & ": " & Err.Description
End Sub

Function Clamp(ByVal x As Variant, Optional ByVal hi As Double = 1#) As Double
    Clamp = IIf(x > hi, hi, x)
End Function
