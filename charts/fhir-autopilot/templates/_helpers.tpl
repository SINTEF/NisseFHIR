{{/*
Expand the name of the chart.
*/}}
{{- define "fhir-autopilot.name" -}}
{{- default .Chart.Name .Values.nameOverride | trunc 63 | trimSuffix "-" }}
{{- end }}

{{/*
Create a default fully qualified app name.
*/}}
{{- define "fhir-autopilot.fullname" -}}
{{- if .Values.fullnameOverride }}
{{- .Values.fullnameOverride | trunc 63 | trimSuffix "-" }}
{{- else }}
{{- $name := default .Chart.Name .Values.nameOverride }}
{{- if contains $name .Release.Name }}
{{- .Release.Name | trunc 63 | trimSuffix "-" }}
{{- else }}
{{- printf "%s-%s" .Release.Name $name | trunc 63 | trimSuffix "-" }}
{{- end }}
{{- end }}
{{- end }}

{{/*
Create chart name and version as used by the chart label.
*/}}
{{- define "fhir-autopilot.chart" -}}
{{- printf "%s-%s" .Chart.Name .Chart.Version | replace "+" "_" | trunc 63 | trimSuffix "-" }}
{{- end }}

{{/*
Common labels
*/}}
{{- define "fhir-autopilot.labels" -}}
helm.sh/chart: {{ include "fhir-autopilot.chart" . }}
{{ include "fhir-autopilot.selectorLabels" . }}
{{- if .Chart.AppVersion }}
app.kubernetes.io/version: {{ .Chart.AppVersion | quote }}
{{- end }}
app.kubernetes.io/managed-by: {{ .Release.Service }}
{{- end }}

{{/*
Selector labels
*/}}
{{- define "fhir-autopilot.selectorLabels" -}}
app.kubernetes.io/name: {{ include "fhir-autopilot.name" . }}
app.kubernetes.io/instance: {{ .Release.Name }}
{{- end }}

{{/*
Create the name of the service account to use
*/}}
{{- define "fhir-autopilot.serviceAccountName" -}}
{{- if .Values.serviceAccount.create }}
{{- default (include "fhir-autopilot.fullname" .) .Values.serviceAccount.name }}
{{- else }}
{{- default "default" .Values.serviceAccount.name }}
{{- end }}
{{- end }}

{{/*
CloudNativePG cluster name
*/}}
{{- define "fhir-autopilot.cnpgClusterName" -}}
{{- printf "%s-db" (include "fhir-autopilot.fullname" .) }}
{{- end }}

{{/*
Database URL: either from CNPG or external config
*/}}
{{- define "fhir-autopilot.databaseSecretName" -}}
{{- if .Values.cnpg.enabled }}
{{- printf "%s-app" (include "fhir-autopilot.cnpgClusterName" .) }}
{{- else }}
{{- ((.Values.cnpg).externalDatabase).existingSecret | default dict | dig "name" "" }}
{{- end }}
{{- end }}

{{/*
JWT Secret name
*/}}
{{- define "fhir-autopilot.jwtSecretName" -}}
{{- if .Values.config.jwtSecret.create }}
{{- printf "%s-jwt" (include "fhir-autopilot.fullname" .) }}
{{- else }}
{{- .Values.config.jwtSecret.existingSecret.name | default "" }}
{{- end }}
{{- end }}

{{/*
JWT Secret key
*/}}
{{- define "fhir-autopilot.jwtSecretKey" -}}
{{- if .Values.config.jwtSecret.create }}
{{- .Values.config.jwtSecret.key | default "jwt-secret" }}
{{- else }}
{{- .Values.config.jwtSecret.existingSecret.key | default "jwt-secret" }}
{{- end }}
{{- end }}
