.PHONY: help init plan apply destroy ssh logs status update

help:
	@echo "Available commands:"
	@echo "  make init      - Initialize Terraform"
	@echo "  make plan      - Show deployment plan"
	@echo "  make apply     - Deploy infrastructure"
	@echo "  make destroy   - Destroy infrastructure"
	@echo "  make ssh       - SSH into container"
	@echo "  make logs      - View application logs"
	@echo "  make status    - Check service status"
	@echo "  make update    - Update application code"
	@echo "  make output    - Show Terraform outputs"

init:
	cd terraform && terraform init

plan:
	cd terraform && terraform plan

apply:
	cd terraform && terraform apply

destroy:
	cd terraform && terraform destroy

ssh:
	@./scripts/ssh.sh

logs:
	@./scripts/logs.sh

status:
	@./scripts/status.sh

update:
	@./scripts/update.sh

output:
	cd terraform && terraform output