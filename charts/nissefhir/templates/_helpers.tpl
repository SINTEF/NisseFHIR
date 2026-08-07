{{/*
Expand the name of the chart.
*/}}
{{- define "nissefhir.name" -}}
{{- default .Chart.Name .Values.nameOverride | trunc 63 | trimSuffix "-" }}
{{- end }}

{{/*
Create a default fully qualified app name.
*/}}
{{- define "nissefhir.fullname" -}}
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
{{- define "nissefhir.chart" -}}
{{- printf "%s-%s" .Chart.Name .Chart.Version | replace "+" "_" | trunc 63 | trimSuffix "-" }}
{{- end }}

{{/*
Common labels
*/}}
{{- define "nissefhir.labels" -}}
helm.sh/chart: {{ include "nissefhir.chart" . }}
{{ include "nissefhir.selectorLabels" . }}
{{- if .Chart.AppVersion }}
app.kubernetes.io/version: {{ .Chart.AppVersion | quote }}
{{- end }}
app.kubernetes.io/managed-by: {{ .Release.Service }}
{{- end }}

{{/*
Selector labels
*/}}
{{- define "nissefhir.selectorLabels" -}}
app.kubernetes.io/name: {{ include "nissefhir.name" . }}
app.kubernetes.io/instance: {{ .Release.Name }}
{{- end }}

{{/*
Create the name of the service account to use
*/}}
{{- define "nissefhir.serviceAccountName" -}}
{{- if .Values.serviceAccount.create }}
{{- default (include "nissefhir.fullname" .) .Values.serviceAccount.name }}
{{- else }}
{{- default "default" .Values.serviceAccount.name }}
{{- end }}
{{- end }}

{{/*
CloudNativePG cluster name
*/}}
{{- define "nissefhir.cnpgClusterName" -}}
{{- printf "%s-db" (include "nissefhir.fullname" .) }}
{{- end }}

{{/*
Database URL: either from CNPG or external config
*/}}
{{- define "nissefhir.databaseSecretName" -}}
{{- if .Values.cnpg.enabled }}
{{- printf "%s-app" (include "nissefhir.cnpgClusterName" .) }}
{{- else }}
{{- ((.Values.cnpg).externalDatabase).existingSecret | default dict | dig "name" "" }}
{{- end }}
{{- end }}

{{/*
JWT Secret name.
An explicit existingSecret.name always wins over create, so documenting or
setting an existing Secret never silently falls back to a generated one.
*/}}
{{- define "nissefhir.jwtSecretName" -}}
{{- $existing := .Values.config.jwtSecret.existingSecret | default dict -}}
{{- if $existing.name }}
{{- $existing.name }}
{{- else if .Values.config.jwtSecret.create }}
{{- printf "%s-jwt" (include "nissefhir.fullname" .) }}
{{- end }}
{{- end }}

{{/*
JWT Secret key
*/}}
{{- define "nissefhir.jwtSecretKey" -}}
{{- $existing := .Values.config.jwtSecret.existingSecret | default dict -}}
{{- if $existing.name }}
{{- $existing.key | default "jwt-secret" }}
{{- else }}
{{- .Values.config.jwtSecret.key | default "jwt-secret" }}
{{- end }}
{{- end }}
